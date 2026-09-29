//! What a builder needs to know about one server.

use tinymcp_bus::{ConnectedServerOverview, McpTool};
use tinytools::ToolExposure;

use crate::config_servers::McpServerDefinition;

/// How a server's tools enter a model's catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpExposure {
    /// The exposure for a tool not named in [`Self::direct_tools`].
    pub default: ToolExposure,
    /// Tools always sent to the model, whatever the default.
    pub direct_tools: Vec<String>,
}

impl McpExposure {
    /// Every tool found through tool search. The default: a server can
    /// advertise dozens of tools, and sending them all every turn costs
    /// context the model rarely needs.
    #[must_use]
    pub fn deferred() -> Self {
        Self {
            default: ToolExposure::Deferred,
            direct_tools: Vec::new(),
        }
    }

    /// Every tool sent to the model every turn.
    #[must_use]
    pub fn direct() -> Self {
        Self {
            default: ToolExposure::Direct,
            direct_tools: Vec::new(),
        }
    }

    /// Deferred, except `tools`, which are sent every turn.
    #[must_use]
    pub fn deferred_except(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            default: ToolExposure::Deferred,
            direct_tools: tools.into_iter().map(Into::into).collect(),
        }
    }

    /// The exposure for the tool named `tool` on the server.
    #[must_use]
    pub fn for_tool(&self, tool: &str) -> ToolExposure {
        if self.direct_tools.iter().any(|name| name.trim() == tool) {
            ToolExposure::Direct
        } else {
            self.default
        }
    }
}

impl Default for McpExposure {
    fn default() -> Self {
        Self::deferred()
    }
}

/// One server's tools, and how to name, describe and expose them.
#[derive(Debug, Clone)]
pub struct McpToolSource {
    /// How the invoker addresses the server: an install identifier, or a
    /// configured name.
    pub server_id: String,
    /// What the server part of each tool name is made from; see
    /// [`crate::tools::naming::server_slug`].
    pub label: String,
    /// The group a tool-search index files these tools under — the server's
    /// qualified or configured name.
    pub family: String,
    /// The name shown in each tool's description.
    pub display_name: String,
    /// The tools, as advertised. The builder sanitizes what it reads; a host
    /// that screens tools (for prompt injection, say) filters this first.
    pub tools: Vec<McpTool>,
    /// How the tools are exposed.
    pub exposure: McpExposure,
}

impl McpToolSource {
    /// An installed server, from its overview.
    ///
    /// Named from its qualified name, so `@scope/ticktick-mcp` gives
    /// `mcp_ticktick_*`.
    #[must_use]
    pub fn from_overview(overview: &ConnectedServerOverview) -> Self {
        Self {
            server_id: overview.server_id.clone(),
            label: overview.qualified_name.clone(),
            family: overview.qualified_name.clone(),
            display_name: overview.display_name.clone(),
            tools: overview.tools.clone(),
            exposure: McpExposure::default(),
        }
    }

    /// A configured server, with the tools listed for it.
    #[must_use]
    pub fn from_definition(definition: &McpServerDefinition, tools: Vec<McpTool>) -> Self {
        Self {
            server_id: definition.name.clone(),
            label: definition.name.clone(),
            family: definition.name.clone(),
            display_name: definition.name.clone(),
            tools,
            exposure: McpExposure::default(),
        }
    }

    /// The same source with `exposure`.
    #[must_use]
    pub fn with_exposure(mut self, exposure: McpExposure) -> Self {
        self.exposure = exposure;
        self
    }
}
