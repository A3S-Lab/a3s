---
name: builtin-tools
description: Built-in file operation and shell tools for A3S
version: 1.0.0
tools:
  - name: read
    description: Read file contents with line numbers
    backend:
      type: binary
      path: a3s-tools
      args_template: "read"
    parameters:
      type: object
      properties:
        file_path:
          type: string
          description: Path to the file to read
        offset:
          type: integer
          description: Line offset (0-indexed)
        limit:
          type: integer
          description: Max lines to read
      required:
        - file_path

  - name: write
    description: Write content to a file
    backend:
      type: binary
      path: a3s-tools
      args_template: "write"
    parameters:
      type: object
      properties:
        file_path:
          type: string
          description: Path to the file to write
        content:
          type: string
          description: Content to write to the file
      required:
        - file_path
        - content

  - name: edit
    description: Edit a file with string replacement
    backend:
      type: binary
      path: a3s-tools
      args_template: "edit"
    parameters:
      type: object
      properties:
        file_path:
          type: string
          description: Path to the file to edit
        old_string:
          type: string
          description: String to find and replace
        new_string:
          type: string
          description: Replacement string
        replace_all:
          type: boolean
          description: Replace all occurrences (default false)
      required:
        - file_path
        - old_string
        - new_string

  - name: bash
    description: Execute a bash command
    backend:
      type: binary
      path: a3s-tools
      args_template: "bash"
    parameters:
      type: object
      properties:
        command:
          type: string
          description: Command to execute
        timeout:
          type: integer
          description: Timeout in milliseconds
      required:
        - command

  - name: grep
    description: Search file contents with ripgrep-style patterns
    backend:
      type: binary
      path: a3s-tools
      args_template: "grep"
    parameters:
      type: object
      properties:
        pattern:
          type: string
          description: Search pattern (regex)
        path:
          type: string
          description: Path to search in (default current directory)
        glob:
          type: string
          description: File glob pattern to filter files
        context:
          type: integer
          description: Number of context lines to show
        ignore_case:
          type: boolean
          description: Case-insensitive search
      required:
        - pattern

  - name: glob
    description: Find files matching a glob pattern
    backend:
      type: binary
      path: a3s-tools
      args_template: "glob"
    parameters:
      type: object
      properties:
        pattern:
          type: string
          description: Glob pattern to match files
        path:
          type: string
          description: Base path to search from (default current directory)
      required:
        - pattern

  - name: ls
    description: List directory contents
    backend:
      type: binary
      path: a3s-tools
      args_template: "ls"
    parameters:
      type: object
      properties:
        path:
          type: string
          description: Path to list (default current directory)
        all:
          type: boolean
          description: Show hidden files
        long:
          type: boolean
          description: Use long listing format
      required: []
---

# Built-in Tools

Core file operation and shell tools for A3S.

## Tools

### read
Read file contents with line numbers. Supports offset and limit for pagination.

### write
Write content to a file. Creates parent directories if needed.

### edit
Edit a file by replacing strings. Supports single or all occurrences.

### bash
Execute shell commands with timeout support.

### grep
Search file contents using regex patterns. Supports context lines and case-insensitive search.

### glob
Find files matching glob patterns.

### ls
List directory contents with optional hidden files and long format.
