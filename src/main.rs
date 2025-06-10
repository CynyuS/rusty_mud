use r3bl_rs_utils_core::friendly_random_id;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{tcp::WriteHalf, TcpStream, TcpListener}, 
    sync::{broadcast::{self, Sender}}
};
use std::{io::{Result, Error, ErrorKind}, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct MsgType {
    pub socket_addr: SocketAddr,
    pub payload: String,
    pub from_id: String,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let addr = "127.0.0.1:7878";

    femme::start(); // start logging

    let tcp_listener = TcpListener::bind(addr).await?; // create the tcp listener
    log::info!("Server is ready to accept connections at {} address!", addr); 

    // create channel where clients can connect to server
    let (tx, _) = broadcast::channel::<MsgType>(10);

    //server loop
    loop {
        // accept incoming connections
        let (tcp_stream, socket_addr) = tcp_listener.accept().await?;

        let tx = tx.clone();
        // spawn creates a tokio task, which switches only when there are awaits
        tokio::spawn(async move {
            let result = handel_client_task(tcp_stream, tx, socket_addr).await;
            match result {
                Ok(_) => {
                    log::info!("handle client task terminated gracefully")
                }
                Err(error) => {
                    log::error!("handle client task encountered error: {}", error)
                },
            }
        });
    }

}

async fn handel_client_task(
    mut tcp_stream: TcpStream, 
    tx: Sender<MsgType>, 
    socket_addr : SocketAddr,
) -> Result<()> {
    log::info!("Handle socket connection from client!");

    let id = friendly_random_id::generate_friendly_random_id();
    let mut rx = tx.subscribe();

    // setup buffer reader/writer
    let (reader, writer) = tcp_stream.split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    let welcome_msg = &format!("Welcome cutie patooties: addr: {}, id: {}\n", 
        socket_addr, id);
    writer.write(welcome_msg.as_bytes()).await?;
    writer.flush().await?;

    let mut incoming = String::new();

    loop {
        let tx = tx.clone();
        tokio::select! {
            //read from broadcast channel
            result = rx.recv() => {
                let result = result.map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
                read_from_broadcast_channel(Ok(result), socket_addr, &mut writer, &id).await?;
            }

            // read from socket
            socket_read = reader.read_line(&mut incoming) => {
                let num_bytes_read : usize = socket_read?;
                // end of file check
                if num_bytes_read == 0{
                    break;
                }
                handle_socket_read(num_bytes_read, &id, &incoming, &mut writer, tx, socket_addr).await?;
                incoming.clear();
            }
        }
    }

    Ok(())
}

async fn read_from_broadcast_channel(
    result: Result<MsgType>,
    socket_addr : SocketAddr,
    writer: &mut BufWriter<WriteHalf<'_>>,
    id: &str,
) -> Result<()> {
    match result {
        Ok(it) => {
            let msg : MsgType = it;
            log::info!("[{}]: channel: {:?}", id, msg);
            if msg.socket_addr != socket_addr {
                let mut message = Vec::new();
                message.extend_from_slice(format!("[{}]:", msg.from_id).as_bytes());
                message.extend_from_slice(msg.payload.as_bytes());

                writer.write_all(&message).await?;
                writer.flush().await?;
            }
        }
        Err(error) => {
            log::error!("{:?}", error);
        }
    }
    
    Ok(())
}

async fn handle_socket_read(
    num_bytes_read: usize,
    id: &str,
    incoming: &str,
    writer: &mut BufWriter<WriteHalf<'_>>,
    tx: Sender<MsgType>,
    socket_addr : SocketAddr,
) -> Result<()> {
    log::info!(
        "[{}]: incoming: {}, size: {}",
        id,
        incoming.trim(),
        num_bytes_read
    );

    // process incoming to outgoing
    let outgoing = &incoming;

    // outgoing turn into writer
    let mut message = Vec::new();
    message.extend_from_slice(b"\x1B[1A\x1B[2K");
    message.extend_from_slice(format!("[{}]:", id).as_bytes());
    message.extend_from_slice(outgoing.as_bytes());

    writer.write_all(&message).await?;
    writer.flush().await?;

    // broadcast outgoing to channel
    let _ = tx.send(MsgType {
        socket_addr,
        payload: incoming.to_string(),
        from_id: id.to_string(),
    });

    log::info!(
        "[{}]: outgoing: {}, size: {}",
        id,
        outgoing.trim(),
        num_bytes_read
    );

    Ok(())
}