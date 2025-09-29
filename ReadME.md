# ChainSmoker

A lightweight Solana shred streaming client. Connect to Solana's gossip network, receive shreds, and pipe them to custom output plugins.

## Overview

ChainSmoker is a library that:
- Connects to Solana testnet/mainnet via gossip protocol
- Receives shreds from validators via TVU Address
- Provides a plugin interface for custom shred processing

This is not a full validator/RPC node. It passively listens to the network without participating in consensus. 

It doesn't replay transaction. It doesnt have an accountdb/ledger persistence. It is just an interface to get shred from turbine and pass on to plugins which can be grpc/quic or custom.

## Architecture
```
Solana Validators
                ┌──────────────────────────┐
                │                          │
                │  Validator 1   TVU:8001  │
                │  Validator 2   TVU:8001  │
                │  Validator 3   TVU:8001  │
                │                          │
                └──────────┬───────────────┘
                           │
                           │ Shreds
                           │
                ┌──────────▼───────────┐
                │   ChainSmoker Node   │
                │                      │
                │   Gossip:  8001      │ ← Peer Discovery
                │   TVU:     8000      │ ← Shred Reception
                └──────────┬───────────┘
                           │
                ┌──────────▼───────────┐
                │   ShredReceiver      │
                │   (Parse UDP)        │
                └──────────┬───────────┘
                           │
                           │ mpsc channel
                           │
                ┌──────────▼───────────┐
                │   PluginRunner       │
                │   (1:N Fanout)       │
                └──┬────────┬────────┬─┘
                   │        │        │
       ┌───────────┘        │        └───────────┐
       │                    │                    │
 ┌─────▼─────┐       ┌─────▼─────┐       ┌─────▼─────┐
 │   gRPC    │       │   QUIC    │       │  Custom   │
 │  Plugin   │       │  Plugin   │       │  Plugin   │
 │  :50051   │       │  :50052   │       │           │
 └─────┬─────┘       └─────┬─────┘       └─────┬─────┘
       │                   │                    │
 N Clients           N Clients            Your Logic
 ```

 