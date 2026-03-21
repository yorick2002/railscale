# Railscale

A scriptable, Tailscale-native network service. Each **carriage** is an independent Tailscale node with its own hostname, running a configurable proxy forwarding pipeline. Behavior is scripted via Lua.

## Architecture

```mermaid
graph TB
    subgraph Tailnet
        C1[carriage-alpha.tailnet.ts.net]
        C2[carriage-beta.tailnet.ts.net]
        C3[carriage-gamma.tailnet.ts.net]
        DNS[dns.tailnet.ts.net]
    end

    C1 --> P1[Proxy Pipeline]
    C2 --> P2[Proxy Pipeline]
    C3 --> P3[Proxy Pipeline]
    DNS --> R[DNS Forwarder]

    subgraph "Carriage Pipeline"
        direction LR
        L[CarriageListener] --> D[DataFrameProducer] --> F[FrameConductor] --> E[DisembarkStrategy]
    end

    style Tailnet fill:#1a1a2e,color:#fff
    style DNS fill:#2d4a22,color:#fff
```

## Carriage Pipeline

Each carriage runs the same four-stage pipeline:

```mermaid
graph LR
    A[Ingress] -->|TCP / HTTP / TLS| B(CarriageListener)
    B -->|raw stream| C(DataFrameProducer)
    C -->|parsed frames| D(FrameConductor)
    D -->|inspected frames| E(DisembarkStrategy)
    E -->|forwarded| F[Upstream Target]

    D -.-|pass/reject| G[Lua Script]
```

| Stage | Trait | Role |
|-------|-------|------|
| **Listen** | `CarriageListener` | Accept connections on this carriage's tailnet address |
| **Parse** | `DataFrameProducer` | Read and buffer frames from the connection |
| **Inspect** | `FrameConductor` | Evaluate frames against rules (Lua-scriptable) |
| **Forward** | `DisembarkStrategy` | Write frames to the upstream destination |

## Key Concepts

- **Carriage** -- a self-contained Tailscale service with its own hostname, its own listener, and its own forwarding rules
- **DNS Server** -- runs as a separate service, not part of the carriage pipeline
- **Lua scripting** -- defines per-carriage behavior: routing, filtering, inspection logic
- **rsnet** -- Rust Tailscale integration, each carriage gets its own `rsnet` instance
