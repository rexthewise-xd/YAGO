use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

#[derive(Debug)]
pub enum InstanceResult {
    Primary(InstanceGuard),
    ForwardedToExisting,
}

#[derive(Debug)]
pub struct InstanceGuard {
    pub port_path: PathBuf,
    pub deep_link_rx: Option<UnboundedReceiver<String>>,
}

impl InstanceGuard {
    pub fn take_receiver(&mut self) -> Option<UnboundedReceiver<String>> {
        self.deep_link_rx.take()
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.port_path);
    }
}

/// Acquires a single-instance lock on the given application data directory.
/// If an existing instance is active, forwards the optional deep link and returns ForwardedToExisting.
pub async fn acquire_or_forward(app_data_dir: &Path, deep_link: Option<&str>) -> InstanceResult {
    let port_path = app_data_dir.join("yago.instance.port");

    // 1. Check if existing instance is active
    if port_path.exists() {
        if let Ok(content) = fs::read_to_string(&port_path) {
            let mut lines = content.lines();
            if let Some(port_str) = lines.next() {
                if let Ok(port) = port_str.trim().parse::<u16>() {
                    let pid_active = if let Some(pid_str) = lines.next() {
                        if let Ok(pid) = pid_str.trim().parse::<u32>() {
                            is_pid_alive(pid)
                        } else {
                            true
                        }
                    } else {
                        true
                    };

                    if pid_active {
                        let addr = format!("127.0.0.1:{}", port);
                        if let Ok(Ok(mut stream)) = tokio::time::timeout(
                            Duration::from_millis(600),
                            TcpStream::connect(&addr),
                        )
                        .await
                        {
                            let msg = if let Some(link) = deep_link {
                                format!("DEEPLINK {}\n", link)
                            } else {
                                "FOCUS\n".to_string()
                            };

                            if stream.write_all(msg.as_bytes()).await.is_ok()
                                && stream.flush().await.is_ok()
                            {
                                return InstanceResult::ForwardedToExisting;
                            }
                        }
                    }
                }
            }
        }
        // Stale port file, remove it
        let _ = fs::remove_file(&port_path);
    }

    // 2. Bind ephemeral port on loopback
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind single-instance listener: {}", e);
            let (_tx, rx) = unbounded_channel();
            return InstanceResult::Primary(InstanceGuard {
                port_path,
                deep_link_rx: Some(rx),
            });
        }
    };

    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => {
            eprintln!("Failed to get listener local_addr: {}", e);
            let (_tx, rx) = unbounded_channel();
            return InstanceResult::Primary(InstanceGuard {
                port_path,
                deep_link_rx: Some(rx),
            });
        }
    };

    let pid = std::process::id();
    let port_content = format!("{}\n{}", port, pid);
    let _ = fs::write(&port_path, port_content);

    let (tx, rx) = unbounded_channel();

    // If a deep link was passed to this primary process on launch, queue it
    if let Some(link) = deep_link {
        let _ = tx.send(link.to_string());
    }

    // Spawn background connection accept loop
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, _)) => {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let mut reader = BufReader::new(socket);
                        let mut line = String::new();
                        if let Ok(n) = reader.read_line(&mut line).await {
                            if n > 0 {
                                let trimmed = line.trim();
                                if let Some(uri) = trimmed.strip_prefix("DEEPLINK ") {
                                    let _ = tx.send(uri.trim().to_string());
                                } else if trimmed == "FOCUS" {
                                    // Focus message received
                                }
                            }
                        }
                    });
                }
                Err(_) => break,
            }
        }
    });

    InstanceResult::Primary(InstanceGuard {
        port_path,
        deep_link_rx: Some(rx),
    })
}

fn is_pid_alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(unix)]
    {
        Path::new(&format!("/proc/{}", pid)).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_acquire_and_forward_to_existing() {
        let dir = tempdir().unwrap();
        let app_data = dir.path().to_path_buf();

        // 1. First instance acquires Primary
        let res1 = acquire_or_forward(&app_data, None).await;
        let mut guard = match res1 {
            InstanceResult::Primary(g) => g,
            InstanceResult::ForwardedToExisting => panic!("Expected Primary for first instance"),
        };
        assert!(guard.port_path.exists());

        // 2. Second instance forwards URI and returns ForwardedToExisting
        let deep_link_uri = "yago://gamebanana/mod/99999";
        let res2 = acquire_or_forward(&app_data, Some(deep_link_uri)).await;
        match res2 {
            InstanceResult::ForwardedToExisting => {}
            InstanceResult::Primary(_) => {
                panic!("Expected ForwardedToExisting for second instance")
            }
        }

        // 3. Primary receives forwarded URI
        let mut rx = guard.take_receiver().expect("Receiver missing");
        let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timed out waiting for forwarded deep link")
            .expect("Channel closed unexpectedly");
        assert_eq!(received, deep_link_uri);

        // 4. Drop primary guard -> port file removed
        drop(guard);
        assert!(!app_data.join("yago.instance.port").exists());
    }
}
