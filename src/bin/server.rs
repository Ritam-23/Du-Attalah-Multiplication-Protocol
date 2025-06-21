use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use rand::Rng;
use std::fs;
use tokio::time::{timeout, Duration};

const PRIME_MOD: i64 = 1e9 as i64 + 7; 
const READ_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let socket1 = "/tmp/da_client1";
    let socket2 = "/tmp/da_client2";

    let _ = fs::remove_file(socket1);
    let _ = fs::remove_file(socket2);

    let listener1 = UnixListener::bind(socket1)?;
    let listener2 = UnixListener::bind(socket2)?;

    println!("Server waiting for connections...");

    // Accept connections with timeout
    let stream1 = match timeout(READ_TIMEOUT, listener1.accept()).await {
        Ok(Ok((stream, _))) => stream,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client1 connection"))
    };
    
    let stream2 = match timeout(READ_TIMEOUT, listener2.accept()).await {
        Ok(Ok((stream, _))) => stream,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client2 connection"))
    };
    
    println!("Both clients connected!");

    let (mut read1, mut write1) = stream1.into_split();
    let (mut read2, mut write2) = stream2.into_split();

    // Generate random masks
    let r0: i64 = rand::rng().random_range(1..PRIME_MOD);
    let r1: i64 = rand::rng().random_range(1..PRIME_MOD);

    println!("Generated masks: r0 = {}, r1 = {}", r0, r1);

    // Initialize buffers and readers
    let mut buf1 = String::new();
    let mut buf2 = String::new();
    let mut reader1 = BufReader::new(&mut read1);
    let mut reader2 = BufReader::new(&mut read2);

    println!("Reading shares from client1...");
    match timeout(READ_TIMEOUT, reader1.read_line(&mut buf1)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client1 disconnected")),
        Ok(Ok(_)) => {
            if buf1.trim().is_empty() {
                return Err(anyhow::anyhow!("Client1 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client1 data"))
    }
    
    let x0: i64 = buf1.trim().parse()?;
    buf1.clear();
    
    match timeout(READ_TIMEOUT, reader1.read_line(&mut buf1)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client1 disconnected")),
        Ok(Ok(_)) => {
            if buf1.trim().is_empty() {
                return Err(anyhow::anyhow!("Client1 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client1 data"))
    }
    
    let y0: i64 = buf1.trim().parse()?;

    println!("Reading shares from client2...");
    match timeout(READ_TIMEOUT, reader2.read_line(&mut buf2)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client2 disconnected")),
        Ok(Ok(_)) => {
            if buf2.trim().is_empty() {
                return Err(anyhow::anyhow!("Client2 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client2 data"))
    }
    
    let x1: i64 = buf2.trim().parse()?;
    buf2.clear();
    
    match timeout(READ_TIMEOUT, reader2.read_line(&mut buf2)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client2 disconnected")),
        Ok(Ok(_)) => {
            if buf2.trim().is_empty() {
                return Err(anyhow::anyhow!("Client2 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client2 data"))
    }
    
    let y1: i64 = buf2.trim().parse()?;

    println!("Received: x0={}, y0={}, x1={}, y1={}", x0, y0, x1, y1);


    let x0_mod = x0 % PRIME_MOD;
    let y0_mod = y0 % PRIME_MOD;
    let x1_mod = x1 % PRIME_MOD;
    let y1_mod = y1 % PRIME_MOD;
    
    // Compute masked values
    let masked_x0 = (x0_mod + r0) % PRIME_MOD;
    let masked_y0 = (y0_mod + r0) % PRIME_MOD;
    let masked_x1 = (x1_mod + r1) % PRIME_MOD;
    let masked_y1 = (y1_mod + r1) % PRIME_MOD;

    // Send masked values to respective clients
    println!("Sending masked values to clients...");
    println!("Sending to client1: x0+r0={}, y0+r0={}", masked_x0, masked_y0);
    match timeout(READ_TIMEOUT, write1.write_all(format!("{}\n{}\n", masked_x0, masked_y0).as_bytes())).await {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout sending data to client1"))
    }
    
    println!("Sending to client2: x1+r1={}, y1+r1={}", masked_x1, masked_y1);
    match timeout(READ_TIMEOUT, write2.write_all(format!("{}\n{}\n", masked_x1, masked_y1).as_bytes())).await {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout sending data to client2"))
    }

    // Wait for clients to exchange data and send back the exchanged masked values
    buf1.clear();
    buf2.clear();
    
    println!("Waiting for exchanged masked values from client1...");

    match timeout(READ_TIMEOUT, reader1.read_line(&mut buf1)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client1 disconnected")),
        Ok(Ok(_)) => {
            if buf1.trim().is_empty() {
                return Err(anyhow::anyhow!("Client1 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client1 data"))
    }
    
    let mx1_from_client1: i64 = buf1.trim().parse()?;
    buf1.clear();
    
    match timeout(READ_TIMEOUT, reader1.read_line(&mut buf1)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client1 disconnected")),
        Ok(Ok(_)) => {
            if buf1.trim().is_empty() {
                return Err(anyhow::anyhow!("Client1 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client1 data"))
    }
    
    let my1_from_client1: i64 = buf1.trim().parse()?;

    println!("Waiting for exchanged masked values from client2...");

    match timeout(READ_TIMEOUT, reader2.read_line(&mut buf2)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client2 disconnected")),
        Ok(Ok(_)) => {
            if buf2.trim().is_empty() {
                return Err(anyhow::anyhow!("Client2 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client2 data"))
    }
    
    let mx0_from_client2: i64 = buf2.trim().parse()?;
    buf2.clear();
    
    match timeout(READ_TIMEOUT, reader2.read_line(&mut buf2)).await {
        Ok(Ok(0)) => return Err(anyhow::anyhow!("Client2 disconnected")),
        Ok(Ok(_)) => {
            if buf2.trim().is_empty() {
                return Err(anyhow::anyhow!("Client2 sent empty data"));
            }
        },
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(anyhow::anyhow!("Timeout waiting for client2 data"))
    }
    
    let my0_from_client2: i64 = buf2.trim().parse()?;


    // DU-ATALLAH MULTIPLICATION PROTOCOL IMPLEMENTATION
    println!("=== DU-ATALLAH MULTIPLICATION PROTOCOL ===");
    
    
    // Direct terms: x0*y0 and x1*y1 (server has direct access)
    let term1 = (x0_mod * y0_mod) % PRIME_MOD;
    let term4 = (x1_mod * y1_mod) % PRIME_MOD;
    
    

    let recovered_y1 = ((my1_from_client1 - r1) % PRIME_MOD + PRIME_MOD) % PRIME_MOD;
    
    let recovered_y0 = ((my0_from_client2 - r0) % PRIME_MOD + PRIME_MOD) % PRIME_MOD;
    
    let term2 = (x0_mod * recovered_y1) % PRIME_MOD;
    let term3 = (x1_mod * recovered_y0) % PRIME_MOD;
   
    let mut final_result = term1;
    final_result = (final_result + term2) % PRIME_MOD;
    final_result = (final_result + term3) % PRIME_MOD;
    final_result = (final_result + term4) % PRIME_MOD;
    
    println!("Du-Atallah aggregation:");
    println!("Du-Atallah result: {}", final_result);
    
    

    Ok(())
}
