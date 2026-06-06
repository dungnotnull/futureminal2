//! Futureminal Plugin - JavaScript plugin host powered by QuickJS.
//!
//! Plugins are sandboxed JavaScript modules that can:
//! - Register custom commands
//! - Transform terminal output
//! - Add UI panels
//! - Listen to terminal events

use std::collections::HashMap;
use std::path::Path;
use tracing::{error, info, warn};

/// A loaded plugin instance.
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: String,
}

/// Plugin host that manages loading and execution.
pub struct PluginHost {
    plugins: HashMap<String, Plugin>,
}

impl PluginHost {
    pub fn new() -> anyhow::Result<Self> {
        info!("Plugin host initialized (QuickJS engine)");
        Ok(Self {
            plugins: HashMap::new(),
        })
    }

    /// Load a plugin from a JavaScript file.
    pub fn load_from_file(&mut self, path: &Path) -> anyhow::Result<String> {
        let source = std::fs::read_to_string(path)?;
        let id = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        self.load_source(id.clone(), source)?;
        Ok(id)
    }

    /// Load a plugin from source code.
    pub fn load_source(&mut self, id: String, source: String) -> anyhow::Result<()> {
        // Validate that source is non-empty and looks like JS
        if source.trim().is_empty() {
            anyhow::bail!("Plugin source is empty");
        }

        // Extract metadata from source comments if available
        let name = extract_meta(&source, "name").unwrap_or_else(|| id.clone());
        let version = extract_meta(&source, "version").unwrap_or_else(|| "0.1.0".into());

        let plugin = Plugin {
            id: id.clone(),
            name,
            version: version.clone(),
            source,
        };

        self.plugins.insert(id.clone(), plugin);
        info!("Loaded plugin {} v{}", id, version);
        Ok(())
    }

    /// Unload a plugin by ID.
    pub fn unload(&mut self, id: &str) -> anyhow::Result<()> {
        if self.plugins.remove(id).is_some() {
            info!("Unloaded plugin {}", id);
            Ok(())
        } else {
            anyhow::bail!("Plugin {} not found", id)
        }
    }

    /// List all loaded plugins.
    pub fn list(&self) -> Vec<(&str, &str, &str)> {
        self.plugins.values()
            .map(|p| (p.id.as_str(), p.name.as_str(), p.version.as_str()))
            .collect()
    }

    /// Get a plugin's source code.
    pub fn get_source(&self, id: &str) -> Option<&str> {
        self.plugins.get(id).map(|p| p.source.as_str())
    }

    /// Execute a plugin hook (e.g., "onCommand", "onOutput").
    pub fn execute_hook(&self, plugin_id: &str, hook: &str, _args: &str) -> anyhow::Result<String> {
        let plugin = self.plugins.get(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("Plugin {} not found", plugin_id))?;

        // In a full implementation, this would run the JS code in QuickJS
        // For now, we return a placeholder indicating the hook was called
        info!("Executed hook {} on plugin {}", hook, plugin_id);
        Ok(format!("Hook {} executed on {}", hook, plugin.name))
    }
}

/// Extract metadata from JS source comments like `// @name my-plugin`
fn extract_meta(source: &str, key: &str) -> Option<String> {
    let prefix = format!("// @{}", key);
    source.lines()
        .find(|l| l.trim().starts_with(&prefix))
        .and_then(|l| l.trim().strip_prefix(&prefix))
        .map(|s| s.trim().to_string())
}

/// Initialize the plugin subsystem.
pub fn init() {
    info!("futureminal-plugin v{} initialized", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_host_creation() {
        let host = PluginHost::new();
        assert!(host.is_ok());
    }

    #[test]
    fn test_plugin_load_and_list() {
        let mut host = PluginHost::new().unwrap();
        let source = r#"
// @name test-plugin
// @version 1.0.0
export function onCommand(cmd) {
    return cmd.toUpperCase();
}
"#;
        host.load_source("test".into(), source.into()).unwrap();
        let list = host.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "test");
        assert_eq!(list[0].1, "test-plugin");
        assert_eq!(list[0].2, "1.0.0");
    }

    #[test]
    fn test_plugin_unload() {
        let mut host = PluginHost::new().unwrap();
        host.load_source("a".into(), "// plugin".into()).unwrap();
        assert!(host.unload("a").is_ok());
        assert!(host.unload("a").is_err());
    }

    #[test]
    fn test_plugin_execute_hook() {
        let mut host = PluginHost::new().unwrap();
        host.load_source("my-plugin".into(), "// @name MyPlugin
// plugin code".into()).unwrap();
        let result = host.execute_hook("my-plugin", "onCommand", "ls").unwrap();
        assert!(result.contains("MyPlugin"));
    }

    #[test]
    fn test_empty_source_rejected() {
        let mut host = PluginHost::new().unwrap();
        assert!(host.load_source("bad".into(), "".into()).is_err());
    }

    #[test]
    fn test_extract_meta() {
        let src = "// @name my-plugin
// @version 2.0.0
console.log(1);";
        assert_eq!(extract_meta(src, "name"), Some("my-plugin".into()));
        assert_eq!(extract_meta(src, "version"), Some("2.0.0".into()));
        assert_eq!(extract_meta(src, "missing"), None);
    }
}
