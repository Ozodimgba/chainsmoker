/*
 ** Output Plugin System **
: The output plugin system provides a simple interface for streaming parsed shreds
: to downstream consumers. Plugins receive shreds via the `handle_shred` method and
: can distribute them using any protocol (gRPC, QUIC, WebSocket, file, etc).

*  ** OutputPlugin Trait **
: All output plugins must implement four methods:

! +----------------+----------------------------------------------------+
! | Method         | Purpose                                            |
! +----------------+----------------------------------------------------+
! | start()        | Initialize plugin (setup servers, open files, etc) |
! | handle_shred() | Process each incoming shred                        |
! | stop()         | Cleanup plugin resources                           |
! | name()         | Return plugin identifier for logging               |
! +----------------+----------------------------------------------------+

*  ** Plugin Lifecycle **
: Plugins follow a simple lifecycle managed by the PluginRunner:

! 1. start()        -> Plugin initializes (servers start, connections open)
! 2. handle_shred() -> Called repeatedly for each received shred
! 3. stop()         -> Plugin cleanup when program exits

*  ** PluginRunner **
: The PluginRunner manages multiple plugins and distributes shreds to all of them.
: Each shred is sent to every registered plugin via the `handle_shred` method.
: If a plugin errors, it logs a warning but continues sending to other plugins.

*  ** Usage Pattern **
: 
: 1. Create plugin instance
: 2. Add to PluginRunner via `add_plugin()`
: 3. Call `start_all()` to initialize all plugins
: 4. Feed shreds via `handle_shred()` in a loop
: 5. Call `stop_all()` for cleanup

*  ** Example Plugin Implementation **
:
: struct MyPlugin { /* state */ }
: 
: #[async_trait::async_trait]
: impl OutputPlugin for MyPlugin {
:     async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
:         // Setup your server/connection
:         Ok(())
:     }
:     
:     async fn handle_shred(&mut self, shred: Shred) -> Result<(), Box<dyn std::error::Error>> {
:         // Process/forward the shred
:         println!("Slot: {}", shred.slot());
:         Ok(())
:     }
:     
:     async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
:         // Cleanup
:         Ok(())
:     }
:     
:     fn name(&self) -> &str { "MyPlugin" }
: }

*  ** Thread Safety **
: Plugins must be Send + Sync as they may be called from async contexts.
*/

use log::{info, warn};
use solana_ledger::shred::Shred;

#[async_trait::async_trait]
pub trait OutputPlugin: Send + Sync {
    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    async fn handle_shred(&mut self, shred: Shred) -> Result<(), Box<dyn std::error::Error>>;
    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn name(&self) -> &str;
}

pub struct PluginRunner {
    plugins: Vec<Box<dyn OutputPlugin>>,
}

impl PluginRunner {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    pub fn add_plugin(&mut self, plugin: Box<dyn OutputPlugin>) {
        self.plugins.push(plugin);
    }

    pub async fn start_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for plugin in &mut self.plugins {
            plugin.start().await?;
            info!("Started {} plugin", plugin.name());
        }
        Ok(())
    }

    pub async fn handle_shred(&mut self, shred: Shred) {
        for plugin in &mut self.plugins {
            if let Err(e) = plugin.handle_shred(shred.clone()).await {
                warn!("Plugin {} error: {}", plugin.name(), e);
            }
        }
    }

    pub async fn stop_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for plugin in &mut self.plugins {
            plugin.stop().await?;
            info!("Stopped {} plugin", plugin.name());
        }
        Ok(())
    }

    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}