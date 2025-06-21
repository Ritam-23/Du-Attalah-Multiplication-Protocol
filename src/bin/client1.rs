use tokio::net::{UnixStream, UnixListener};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{timeout, Duration};
use std::fs;
use num_bigint::BigUint;

const READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct Secret {
    pub x: BigUint,
    pub y: BigUint,
}

impl Secret {
    pub fn new(x: u64, y: u64) -> Self {
        Secret {
            x: BigUint::from(x),
            y: BigUint::from(y),
        }
    }
    
    pub fn from_strings(x_str: &str, y_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Secret {
            x: x_str.parse::<BigUint>()?,
            y: y_str.parse::<BigUint>()?,
        })
    }
    
    pub fn to_i64_safely(&self) -> Result<(i64, i64), Box<dyn std::error::Error>> {
        let x_bytes = self.x.to_bytes_le();
        let y_bytes = self.y.to_bytes_le();
        
        if x_bytes.len() > 8 || y_bytes.len() > 8 {
            return Err("Number too large for i64".into());
        }
        
        let mut x_array = [0u8; 8];
        let mut y_array = [0u8; 8];
        
        x_array[..x_bytes.len()].copy_from_slice(&x_bytes);
        y_array[..y_bytes.len()].copy_from_slice(&y_bytes);
        
        let x_val = i64::from_le_bytes(x_array);
        let y_val = i64::from_le_bytes(y_array);
        
        Ok((x_val, y_val))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to server
    let server_socket = "/tmp/da_client1";
    println!("Client1: Connecting to server at {}", server_socket);
    
    let server_stream = match timeout(READ_TIMEOUT, UnixStream::connect(server_socket)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout connecting to server"))
    };
    println!("Client1: Connected to server");
    
    let (server_read, mut server_write) = server_stream.into_split();
    let mut server_reader = BufReader::new(server_read);

    // Setup peer-to-peer communication socket
    let p2p_socket = "/tmp/p2p_client1_to_client2";
    let _ = fs::remove_file(p2p_socket);
    let p2p_listener = UnixListener::bind(p2p_socket)?;
    
    // Get user input and create Secret struct
    let stdin = tokio::io::stdin();
    let mut input = BufReader::new(stdin).lines();

    println!("Enter x0 (natural number):");
    let x0_str = match timeout(READ_TIMEOUT, input.next_line()).await {
        Ok(Ok(Some(line))) => {
            if line.trim().is_empty() {
                return Err(anyhow::anyhow!("Empty input"));
            }
            line.trim().to_string()
        },
        Ok(Ok(None)) => return Err(anyhow::anyhow!("No input provided")),
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for input"))
    };
    
    println!("Enter y0 (natural number):");
    let y0_str = match timeout(READ_TIMEOUT, input.next_line()).await {
        Ok(Ok(Some(line))) => {
            if line.trim().is_empty() {
                return Err(anyhow::anyhow!("Empty input"));
            }
            line.trim().to_string()
        },
        Ok(Ok(None)) => return Err(anyhow::anyhow!("No input provided")),
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for input"))
    };

    // Create Secret struct from input
    let client1_secret = Secret::from_strings(&x0_str, &y0_str).unwrap();
    println!("Client1: Created secret struct with x0={}, y0={}", client1_secret.x, client1_secret.y);

    // Convert to i64 for protocol compatibility
    let (x0, y0) = client1_secret.to_i64_safely().unwrap();

    // Send shares to server
    println!("Client1: Sending x0={}, y0={} to server", x0, y0);
    match timeout(READ_TIMEOUT, server_write.write_all(format!("{}\n{}\n", x0, y0).as_bytes())).await {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout sending data to server"))
    }

    // Receive own masked values from server
    println!("Client1: Waiting for masked values from server...");
    let mut mx0 = String::new();
    let mut my0 = String::new();
    
    match timeout(READ_TIMEOUT, server_reader.read_line(&mut mx0)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Server disconnected")),
        Ok(Ok(_)) => {
            if mx0.trim().is_empty() {
                return Err(anyhow::anyhow!("Server sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for server data"))
    }
    
    match timeout(READ_TIMEOUT, server_reader.read_line(&mut my0)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Server disconnected")),
        Ok(Ok(_)) => {
            if my0.trim().is_empty() {
                return Err(anyhow::anyhow!("Server sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for server data"))
    }

    let mx0: i64 = mx0.trim().parse()?;
    let my0: i64 = my0.trim().parse()?;
    
    println!("Client1: Received masked values: mx0={}, my0={}", mx0, my0);

    // Accept connection from Client2 for peer exchange
    println!("Client1: Waiting for Client2 connection...");
    let (p2p_stream, _) = match timeout(READ_TIMEOUT, p2p_listener.accept()).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for Client2 connection"))
    };
    
    let (p2p_read, mut p2p_write) = p2p_stream.into_split();
    let mut p2p_reader = BufReader::new(p2p_read);

    // Send own masked values to Client2
    println!("Client1: Sending masked values to Client2...");
    match timeout(READ_TIMEOUT, p2p_write.write_all(format!("{}\n{}\n", mx0, my0).as_bytes())).await {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout sending data to Client2"))
    }

    // Receive Client2's masked values
    println!("Client1: Receiving masked values from Client2...");
    let mut mx1 = String::new();
    let mut my1 = String::new();
    
    match timeout(READ_TIMEOUT, p2p_reader.read_line(&mut mx1)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client2 disconnected")),
        Ok(Ok(_)) => {
            if mx1.trim().is_empty() {
                return Err(anyhow::anyhow!("Client2 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for Client2 data"))
    }
    
    match timeout(READ_TIMEOUT, p2p_reader.read_line(&mut my1)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client2 disconnected")),
        Ok(Ok(_)) => {
            if my1.trim().is_empty() {
                return Err(anyhow::anyhow!("Client2 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for Client2 data"))
    }

    let mx1: i64 = mx1.trim().parse()?;
    let my1: i64 = my1.trim().parse()?;
    
    println!("Client1: Received Client2's masked values: mx1={}, my1={}", mx1, my1);

    // Send exchanged values back to server
    println!("Client1: Sending exchanged values back to server...");
    match timeout(READ_TIMEOUT, server_write.write_all(format!("{}\n{}\n", mx1, my1).as_bytes())).await {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout sending exchanged values to server"))
    }
    
    println!("Client1: Sent exchanged values to server");
    println!("Client1: Secret struct contained x0={}, y0={}", client1_secret.x, client1_secret.y);
    println!("Client1: Done - Server will compute final result using Du-Atallah protocol");

    Ok(())
}
