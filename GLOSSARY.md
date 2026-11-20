# haulmer-googleworkspace-mcp

A desktop app that lets a Haulmer employee give their AI tools access to their own Google Workspace data.

## Language

### Where people use AI

A product can be more than one kind. Classify it by what it can do on the person's machine, not by its brand. For example, a normal chat in ChatGPT desktop is a **Chat app**, but its Codex mode is an **Agent**.

**Agent**:
An AI tool that can run commands on the person's machine. Examples: Claude Code, Codex, Hermes, Cursor.
_Avoid_: client, assistant, surface

**Chat app**:
A desktop chat app that can use a local MCP server but cannot install one by itself. Examples: Claude Desktop, ChatGPT desktop.
_Avoid_: GUI client, desktop agent

**Web chat**:
A chat that runs in the browser and cannot use a local MCP server at all. Examples: claude.ai, chatgpt.com.
_Avoid_: web client, web agent

### Connecting

**Install prompt**:
The text a person pastes into an **Agent** so that the Agent connects itself to haulmer-googleworkspace-mcp. A **Chat app** cannot act on it, so it turns the prompt into manual steps for the person.
_Avoid_: setup prompt, copy prompt, install instructions

**MCP extension**:
A one-click installer file (`.mcpb`) that haulmer-googleworkspace-mcp generates and that a compatible **Chat app**, such as Claude Desktop, opens to connect itself to haulmer-googleworkspace-mcp.
_Avoid_: DXT, desktop extension, bundle
