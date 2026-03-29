use std::net::Ipv4Addr;
use std::process;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    #[cfg(feature = "tailscale")]
    {
        error!("tailscale listener not yet implemented");
        process::exit(1);
    }

    #[cfg(not(feature = "tailscale"))]
    {
        let http_addr = std::env::var("RAILSCALE_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
        let dns_addr = std::env::var("RAILSCALE_DNS_ADDR").unwrap_or_else(|_| "127.0.0.1:5354".into());
        let upstream_dns = std::env::var("RAILSCALE_DNS_UPSTREAM").unwrap_or_else(|_| "8.8.8.8:53".into());

        let mut resolver = rail_turnout::dns::resolver::DnsResolver::new(
            "intranet.levandor.io".into(),
            upstream_dns.parse().expect("invalid upstream DNS address"),
        );
        resolver.add_rule("postgres", Ipv4Addr::new(192, 168, 1, 13));
        resolver.add_rule("datalake", Ipv4Addr::new(192, 168, 1, 20));

        let http_listener = rail_proto::carriage::nontailscale::DevListener::bind(&http_addr).await;
        let dns_server = rail_proto::dns::server::DnsServer::bind(&dns_addr, resolver).await;

        let http_listener = match http_listener {
            Ok(l) => l,
            Err(e) => {
                error!("failed to bind http {http_addr}: {e}");
                process::exit(1);
            }
        };

        let dns_server = match dns_server {
            Ok(s) => s,
            Err(e) => {
                error!("failed to bind dns {dns_addr}: {e}");
                process::exit(1);
            }
        };

        info!("railscale dev server starting");

        tokio::select! {
            res = http_listener.run() => {
                if let Err(e) = res {
                    error!("http server error: {e}");
                    process::exit(1);
                }
            }
            res = dns_server.run() => {
                if let Err(e) = res {
                    error!("dns server error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
