use std::{sync::Arc, time::Duration, collections::HashMap, pin::Pin};

use futures::{Future, stream::FuturesUnordered, StreamExt};

use tokio::{time::{self, Instant, Timeout, error::Elapsed}, task::{self, JoinHandle}, sync::{Mutex, mpsc}};

use crate::{cli_args::CliArgs, ping_net::PingNet, utils::{emulate_payload_delay, self}, connection_handle::{ConnectionHandle, EchoStatus}, config::ICMP_ECHO_TIMEOUT_DURATION_SECONDS};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunError {
    #[error("EchoReplyFailure")]
    EchoReplyFailure,

    #[error("Received unkown sequence number")]
    UnknownSequenceNumber(u16),

    #[error("Received elapsed reply")]
    ElapsedReply,

    #[error("Received unexpected reply echo status")]
    UnexpectedReplyEchoStatus(EchoStatus),

    #[error("Received packet from unexpected IP")]
    UnexpectedIP(String),
}

type EchoSubTaskResult<T, E = RunError> = std::result::Result<T, E>;

pub type ConnectionHandleMap = HashMap<u16, ConnectionHandle>;

type SendFutureOutputType = ConnectionHandle;
type TimedFutureOutputType = Pin<Box<dyn Future<Output = Result<SendFutureOutputType, Elapsed>> + Send>>;
type WrappedTimedSendFutureOutputType = (u16, TimedFutureOutputType);

type RecvFutureOutputType = (u16, Timeout<JoinHandle<EchoSubTaskResult<ConnectionHandle>>>);

type MpscSendOutputType = Pin<Box<dyn Future<Output=WrappedTimedSendFutureOutputType> + Send>>;
type MpscRecvOutputType = Pin<Box<dyn Future<Output=RecvFutureOutputType> + Send>>;

pub async fn run(args: CliArgs) {
    let destination_ipv4 = args.config.target_addr_ipv4.ip().clone();
    let ping_count = args.config.ping_count;
    let ping_interval = args.config.ping_interval;
    let socket = PingNet::create_socket().unwrap();
    let socket_recv = Arc::clone(&socket);
    let identifier = rand::random::<u16>();

    log::info!("run: identifier: {}, destination: {}, ping_count: {} ping_interval: {}", identifier, destination_ipv4, ping_count, ping_interval);
    
    let futures = FuturesUnordered::new();
    let conn_map: Arc<Mutex<ConnectionHandleMap>> = Arc::new(Mutex::new(HashMap::new()));
    let conn_map_task_send = Arc::clone(&conn_map);
    let conn_map_task_recv = Arc::clone(&conn_map);
    
    //

    let (tx_recv, mut rx_recv) = mpsc::channel::<MpscRecvOutputType>(10);
    let recv_channel_task = task::spawn(async move {
        log::info!("recv_channel_task: started");

        let futures_recv = FuturesUnordered::new();
        
        for _seq_num in 0..ping_count {
            log::info!("recv_channel_task: rx_recv await...");
            if let Some(recv_future_timed_wrapped) = rx_recv.recv().await {
                let recv_future_timed_wrapped = recv_future_timed_wrapped.await;
                let (seq_num_recv, recv_future_timed_future) = recv_future_timed_wrapped;
                log::info!("recv_channel_task: rx_recv await ready: seq_num_recv {}", seq_num_recv);
                
                let conn_map_task_recv = conn_map_task_recv.clone();
                let recv_future_timed_task = task::spawn(async move {
                    let recv_future_timed_result = recv_future_timed_future.await;

                    let recv_future_result = match recv_future_timed_result {
                        Ok(recv_future_result) => recv_future_result,
                        Err(err) => {
                            println!("{},{},time out", destination_ipv4, seq_num_recv);
                            log::warn!("[ELAPSED] task_recv: recv_future_timed_task: seq_num_recv: {}, Elapsed: {:?}", seq_num_recv, err);

                            // Update map - On elapsed
                            let mut conn_map = conn_map_task_recv.lock().await;
                            let seq_status = match conn_map.get_mut(&seq_num_recv) {
                                Some(conn_handle) => {
                                    conn_handle.status = EchoStatus::Elapsed;
                                    conn_handle.status
                                },
                                None => {
                                    log::warn!("recv_channel_task: recv_future_timed_task: seq_num_recv not found: {}, ", seq_num_recv);
                                    EchoStatus::Failed
                                }
                            };
                            
                            return (seq_num_recv, seq_status);
                        }
                    };
                    
                    let recv_result = match recv_future_result {
                        Ok(recv_result) => recv_result,
                        Err(err) => {
                            log::warn!("\ttask_recv: recv_future_timed_task: seq_num_recv: {}, Failed to join a spawned task: {:?}", seq_num_recv, err);
                            
                            let mut conn_map = conn_map_task_recv.lock().await;
                            match conn_map.get_mut(&seq_num_recv) {
                                Some(conn_handle) => {
                                    conn_handle.status = EchoStatus::Failed;
                                    conn_handle.status;
                                },
                                None => {
                                    log::warn!("recv_channel_task: recv_future_timed_task: seq_num_recv not found: {}, ", seq_num_recv);
                                }
                            };
                            
                            return (seq_num_recv, EchoStatus::Failed);
                        },
                    };
                    
                    let recv_subtask_result = match recv_result {
                        Ok(conn_handle_recv) => {
                            println!("{}", conn_handle_recv);
                            log::info!("[DONE] recv_channel_task: recv_future_timed_task: seq_num_recv: {} ready: {}", seq_num_recv, conn_handle_recv);
                            (seq_num_recv, EchoStatus::Received)
                        },
                        Err(err) => {
                            // Update map - On PingNet error
                            let mut conn_map = conn_map_task_recv.lock().await;
                            let conn_handle = conn_map.get_mut(&seq_num_recv).unwrap();
                            let old_status = conn_handle.status.clone();
                            conn_handle.status = EchoStatus::Failed;
                            log::warn!("\trecv_channel_task: recv_future_timed_task: seq_num_recv: {}, Failed recv ICMP reply, err: {}, old_status: {}", seq_num_recv, err, old_status);
                            (seq_num_recv, EchoStatus::Failed)
                        }
                    };

                    recv_subtask_result                    
                });

                futures_recv.push(recv_future_timed_task);
            }
        }

        futures_recv.for_each_concurrent(None, |r| async move {
            match r {
                Ok((seq_num_recv, echo_status)) => log::info!("recv_channel_task: futures_recv: seq_num_recv {} finished with status ({})", seq_num_recv, echo_status),
                Err(e) => log::warn!("task_send: futures_recv: failed with error: {:?}", e),
            }
        }).await;

        log::info!("recv_channel_task: finished");

        "recv_channel_task"
    });
    futures.push(recv_channel_task);

    //
    //
    //
    
    let (tx_send, mut rx_send) = mpsc::channel::<MpscSendOutputType>(10);
    let send_channel_task = task::spawn(async move {
        log::info!("send_channel_task: started");
        
        let futures_send = FuturesUnordered::new();

        for _seq_num in 0..ping_count {
            log::info!("send_channel_task: rx_send await...");
            if let Some(send_future_timed) = rx_send.recv().await {

                let send_future_timed_wrapped = send_future_timed.await;
                let (seq_num_send, send_future_timed_future) = send_future_timed_wrapped;
                log::info!("send_channel_task: rx_send await ready: seq_num_send {}", seq_num_send);
                
                let tx_recv = tx_recv.clone();
                let conn_map_task_send = conn_map_task_send.clone();
                let socket_recv = socket_recv.clone();

                let send_future_timed_task = task::spawn(async move {
                    let send_future_timed_result = send_future_timed_future.await;
                    let connection_handle_send = match send_future_timed_result {
                        Ok(send_future_result) => send_future_result,
                        Err(err) => {
                            println!("{},{},time out", destination_ipv4, seq_num_send);
                            log::warn!("send_channel_task: send_future_timed_task: seq_num_send: {}, Elapsed: {:?}", seq_num_send, err);

                            // Update map - On elapsed
                            let mut conn_map = conn_map_task_send.lock().await;
                            let seq_status = match conn_map.get_mut(&seq_num_send) {
                                Some(conn_handle) => {
                                    conn_handle.status = EchoStatus::Elapsed;
                                    conn_handle.status
                                },
                                None => {
                                    log::warn!("send_channel_task: send_future_timed_task: seq_num_send not found: {}, ", seq_num_send);
                                    EchoStatus::Failed
                                }
                            };

                            return (seq_num_send, seq_status);
                        },
                    };
                    
                    // Update map - On send success
                    {
                        let mut conn_map = conn_map_task_send.lock().await;
                        // let conn_handle_entry = 
                        match conn_map.get_mut(&seq_num_send) {
                            Some(conn_handle) => {
                                conn_handle.status = EchoStatus::Sent;
                                conn_handle.elapsed_micros = connection_handle_send.elapsed_micros;
                            },
                            None => {
                                log::warn!("send_channel_task: send_future_timed_task: seq_num_send not found: {}, ", seq_num_send);
                                return (seq_num_send, EchoStatus::Failed);
                            }
                        }
                    }

                    // Spawn ICMP reply task
                    let recv_task: JoinHandle<EchoSubTaskResult<ConnectionHandle>> = task::spawn(async move {
                        emulate_payload_delay(seq_num_send).await;
                        let recv_reply = PingNet::recv_ping_respond(socket_recv).await;
                        let now = Instant::now();
                        
                        let recv_result = match recv_reply {
                            Some((ip_recv, seq_num_recv)) => {
                                if ip_recv != destination_ipv4 {
                                    return Err(RunError::UnexpectedIP(ip_recv.to_string()))
                                }
                                let mut conn_map_guard = conn_map_task_send.lock().await;
                                match conn_map_guard.get_mut(&seq_num_recv) {
                                    Some(c) if c.status == EchoStatus::Elapsed => {
                                        log::warn!("recv_task: recv_ping_respond: Received reply for the elapsed request with seq_num_recv: {}, ({})", seq_num_recv, c.status);
                                        Err(RunError::ElapsedReply)
                                    }
                                    Some(c) if c.status == EchoStatus::Sent => {
                                        let connection_handle = c.clone().to_received(now);
                                        c.status = EchoStatus::Received;
                                        c.now = now;
                                        c.elapsed_micros = connection_handle.elapsed_micros;
                                        log::info!("recv_task: recv_ping_respond with seq_num_recv: {}, conn_handle: {} == {}", seq_num_recv, connection_handle, c);
                                        Ok(connection_handle)
                                    },
                                    Some(c) => {
                                        log::warn!("recv_task: recv_ping_respond: unexpected status with seq_num_recv: {}, ({})", seq_num_recv, c.status);
                                        Err(RunError::UnexpectedReplyEchoStatus(c.status))
                                    },
                                    None => {
                                        log::warn!("recv_task: recv_ping_respond with unknown sequence number: {}", seq_num_recv);
                                        Err(RunError::UnknownSequenceNumber(seq_num_recv))
                                    },
                                }
                            }
                            _ => Err(RunError::EchoReplyFailure),
                        };
                        recv_result
                    });

                    // Make timed ICMP reply task
                    let recv_task_timed = time::timeout(ICMP_ECHO_TIMEOUT_DURATION_SECONDS, recv_task);

                    // Make wrapped timed ICMP reply future - to identify sequence number of a task that has timed out on the receiver side
                    let recv_task_timed_wrapped = async move {
                        (seq_num_send, recv_task_timed)
                    };

                    // Send wrapped future to the receiver
                    tx_recv.send(Box::pin(recv_task_timed_wrapped)).await.expect("Send Failed");

                    (seq_num_send, EchoStatus::Sent)
                });

                // Save send future
                futures_send.push(send_future_timed_task);
            }
        }

        // Await concurrently on send futures
        futures_send.for_each_concurrent(None, |r| async move {
            match r {
                Ok((seq_num_send, echo_status)) => log::debug!("send_channel_task: futures_send: seq_num_send {} finished with status {}", seq_num_send, echo_status),
                Err(e) => log::warn!("send_channel_task: futures_send: failed with error: {:?}", e),
            }
        }).await;

        log::debug!("send_channel_task: finished");

        "send_channel_task"
    });
    futures.push(send_channel_task);

    //
    //
    //

    let mut interval = time::interval(Duration::from_millis(ping_interval));
    
    for seq_num in 0..ping_count {
        log::info!("Scheduling seq_num: {}...", seq_num);

        let socket = Arc::clone(&socket);
        let conn_map = Arc::clone(&conn_map);

        let send_future = async move {
            let earlier = Instant::now();
            let conn_handle = ConnectionHandle::new(destination_ipv4, seq_num, identifier, earlier, earlier);
            {
                let mut conn_map = conn_map.lock().await;
                let new_entry = conn_map.entry(seq_num).or_insert(conn_handle.clone());
                log::debug!("send_task: new entry: seq_num: {}, status: {}", seq_num, new_entry.status);
            }
            emulate_payload_delay(seq_num).await;
            let conn_handle = PingNet::send_ping_request(socket, conn_handle).expect("Send Failed");
            conn_handle
        };
        let send_task_timed = utils::timeout_future(send_future);
        let send_task_timed_wrapped = async move {
            let send_task_timed_pin: TimedFutureOutputType = Box::pin(send_task_timed);
            (seq_num, send_task_timed_pin)
        };

        let p: MpscSendOutputType = Box::pin(send_task_timed_wrapped);
        tx_send.send(p).await.expect("Send failed");

        interval_tick(seq_num, ping_count, &mut interval).await;
    }

    futures.for_each_concurrent(None, |r| async move {
        match r {
            Ok(task_name) => log::debug!("Top level task finished: {}", task_name),
            Err(e) => log::warn!("Top level task failed with error: {:?}", e),
        }
    }).await;

    dump_conn_map(conn_map).await;
}

async fn interval_tick(seq_num: u16, ping_count: u16, interval: &mut time::Interval) -> Option<Instant> {
    if seq_num < ping_count - 1 {
        Some(interval.tick().await)
    } else {
        None
    }
}

async fn dump_conn_map(conn_map: Arc<Mutex<ConnectionHandleMap>>) {
    log::debug!("dump_conn_map - started");
    for connection_handle in conn_map.lock().await.values() {
        log::debug!("{}", connection_handle);
    }
    log::debug!("dump_conn_map - finished");
}
