use serde_json::{Value, json};

use crate::{
    ASK_QUESTION_TOOL, CREATE_TODO_GRAPH_TOOL, EDIT_FILE_TOOL, FIND_FILES_TOOL,
    GET_TODO_GRAPH_TOOL, GRAPH_EXPLORE_TOOL, GRAPH_FIND_CALLEES_TOOL, GRAPH_FIND_CALLERS_TOOL,
    GRAPH_FIND_REFERENCES_TOOL, GRAPH_FIND_SYMBOLS_TOOL, GRAPH_RELATED_FILES_TOOL, IMAGE_GEN_TOOL,
    READ_FILE_TOOL, RUN_COMMAND_TOOL, SEARCH_TEXT_TOOL, SLEEP_TOOL, ToolDefinition,
    UPDATE_TODO_GRAPH_TOOL, WEB_FETCH_TOOL, WEB_SEARCH_TOOL, WRITE_FILE_TOOL,
};

pub(crate) fn builtin_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_file_definition(),
        find_files_definition(),
        graph_find_symbols_definition(),
        graph_find_callers_definition(),
        graph_find_callees_definition(),
        graph_find_references_definition(),
        graph_related_files_definition(),
        graph_explore_definition(),
        search_text_definition(),
        web_search_definition(),
        web_fetch_definition(),
        image_gen_definition(),
        write_file_definition(),
        edit_file_definition(),
        create_todo_graph_definition(),
        update_todo_graph_definition(),
        get_todo_graph_definition(),
        ask_question_definition(),
        run_command_definition(),
        sleep_definition(),
    ]
}

fn read_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: READ_FILE_TOOL,
        description: "Read a text file inside the active workspace, optionally restricted to a 1-based inclusive line range. The returned content is prefixed with real 1-based file line numbers for edit targeting; line-number prefixes are not file content and must not be copied into write_file content or edit_file oldStr/newStr values.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative file path."
                },
                "startLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based first line to read. Must be null when endLine is null."
                },
                "endLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based last line to read, inclusive. Values beyond the file length read through the final line. Must be null when startLine is null."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 5000."
                }
            },
            "required": ["path", "startLine", "endLine", "timeoutMs"]
        }),
        strict: true,
    }
}

fn find_files_definition() -> ToolDefinition {
    ToolDefinition {
        name: FIND_FILES_TOOL,
        description: "Find files and directories under a workspace-relative directory using optional glob include/exclude patterns.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative directory path to search recursively. Use . for the workspace root."
                },
                "include": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Optional glob patterns matched against returned workspace-relative paths. Null or an empty array includes everything not excluded."
                },
                "exclude": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Optional glob patterns matched against returned workspace-relative paths to omit."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 5000."
                }
            },
            "required": ["path", "include", "exclude", "timeoutMs"]
        }),
        strict: true,
    }
}

fn graph_find_symbols_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_SYMBOLS_TOOL,
        description: "Find indexed code graph symbol candidates and symbolIds by name, signature, or documentation. Use this for disambiguation or candidate lists; use graph_explore instead when you need source code snippets.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Symbol name or partial text to find."
                },
                "kind": {
                    "type": ["string", "null"],
                    "description": "Optional symbol kind such as function, method, struct, class, enum, trait, variable, or constant."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path to restrict the query."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["query", "kind", "path", "limit", "timeoutMs"]
        }),
        strict: true,
    }
}

fn graph_find_callers_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_CALLERS_TOOL,
        description: "Find code graph caller relationships for the requested symbol. This returns relationship metadata, not source snippets; use graph_explore for source context. Use symbolId from graph_find_symbols when names are ambiguous.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "symbolId": {
                    "type": ["integer", "null"],
                    "description": "Exact code graph symbol id returned by graph_find_symbols."
                },
                "symbol": {
                    "type": ["string", "null"],
                    "description": "Symbol name to resolve when it is unique."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path used only with symbol."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit", "timeoutMs"]
        }),
        strict: true,
    }
}

fn graph_find_callees_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_CALLEES_TOOL,
        description: "Find code graph callee relationships from the requested symbol. This returns relationship metadata, not source snippets; use graph_explore for source context. Use symbolId from graph_find_symbols when names are ambiguous.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "symbolId": {
                    "type": ["integer", "null"],
                    "description": "Exact code graph symbol id returned by graph_find_symbols."
                },
                "symbol": {
                    "type": ["string", "null"],
                    "description": "Symbol name to resolve when it is unique."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path used only with symbol."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit", "timeoutMs"]
        }),
        strict: true,
    }
}

fn graph_find_references_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_REFERENCES_TOOL,
        description: "Find indexed reference locations for the requested symbol. This returns locations, not source snippets; use graph_explore for source context around symbols. Use symbolId from graph_find_symbols when names are ambiguous.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "symbolId": {
                    "type": ["integer", "null"],
                    "description": "Exact code graph symbol id returned by graph_find_symbols."
                },
                "symbol": {
                    "type": ["string", "null"],
                    "description": "Symbol name to resolve when it is unique."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path used only with symbol."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["symbolId", "symbol", "path", "limit", "timeoutMs"]
        }),
        strict: true,
    }
}

fn graph_related_files_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_RELATED_FILES_TOOL,
        description: "Find files related to an indexed workspace file through code graph edges or shared imports. Use this to discover adjacent files, not to read source snippets.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative indexed file path."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 50. Defaults to 20."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["path", "limit", "timeoutMs"]
        }),
        strict: true,
    }
}

fn graph_explore_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_EXPLORE_TOOL,
        description: "Default code graph tool for source context: find indexed code graph symbols and return matching source snippets with real 1-based line numbers. Use this instead of graph_find_symbols plus read_file when you need code for a symbol or likely target.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "symbolId": {
                    "type": ["integer", "null"],
                    "description": "Exact code graph symbol id returned by graph_find_symbols. Provide exactly one of symbolId or query."
                },
                "query": {
                    "type": ["string", "null"],
                    "description": "Symbol name or partial text to find and read. Provide exactly one of query or symbolId."
                },
                "kind": {
                    "type": ["string", "null"],
                    "description": "Optional symbol kind used only with query, such as function, method, struct, class, enum, trait, variable, or constant."
                },
                "path": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative file or directory path used only with query."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Optional result limit from 1 to 20 when using query. Defaults to 5."
                },
                "contextLines": {
                    "type": ["integer", "null"],
                    "description": "Optional number of context lines before and after each symbol, from 0 to 20. Defaults to 2."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["symbolId", "query", "kind", "path", "limit", "contextLines", "timeoutMs"]
        }),
        strict: true,
    }
}

fn search_text_definition() -> ToolDefinition {
    ToolDefinition {
        name: SEARCH_TEXT_TOOL,
        description: "Search workspace text and return matching lines. Powered by ripgrep/rg; the query uses rg pattern syntax. When there are too many matches the response is truncated to the first matches with truncated=true; the complete results are written to a workspace file reported as fullResultPath, which you can read with read_file (or refine the query/path) to see every match.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Ripgrep search pattern."
                },
                "path": {
                    "type": "string",
                    "description": "Workspace-relative path to search. Use . for the workspace root."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["query", "path", "timeoutMs"]
        }),
        strict: true,
    }
}

fn web_search_definition() -> ToolDefinition {
    ToolDefinition {
        name: WEB_SEARCH_TOOL,
        description: "Search the web for current or external information using the search API configured in Foco settings. Use web_fetch on result URLs when page details or direct source text are needed.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query."
                },
                "maxResults": {
                    "type": ["integer", "null"],
                    "description": "Optional number of results from 1 to 10. Defaults to 5."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 15000."
                }
            },
            "required": ["query", "maxResults", "timeoutMs"]
        }),
        strict: true,
    }
}

fn web_fetch_definition() -> ToolDefinition {
    ToolDefinition {
        name: WEB_FETCH_TOOL,
        description: "Fetch an HTTP or HTTPS URL and return readable text content with basic page metadata. For large pages, full fetches fail with an instruction to retry using a 1-based inclusive line range.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "url": {
                    "type": "string",
                    "description": "HTTP or HTTPS URL to fetch."
                },
                "startLine": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "description": "Optional 1-based first readable-text line to return. Must be set together with endLine; null requests the full page."
                },
                "endLine": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "description": "Optional 1-based last readable-text line to return, inclusive. Must be set together with startLine; values beyond the page line count read through the final line."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 15000."
                }
            },
            "required": ["url", "startLine", "endLine", "timeoutMs"]
        }),
        strict: true,
    }
}

fn image_gen_definition() -> ToolDefinition {
    ToolDefinition {
        name: IMAGE_GEN_TOOL,
        description: "Generate or edit images using the configured image generation model. The tool saves generated images under the workspace .foco directory and returns file paths plus metadata; it does not return image bytes inline.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Detailed prompt describing the image to generate or edit."
                },
                "mode": {
                    "type": ["string", "null"],
                    "enum": ["generate", "edit", null],
                    "description": "Image operation mode. Defaults to generate. Edit mode requires at least one input image."
                },
                "model": {
                    "type": ["string", "null"],
                    "description": "Optional configured image-capable model id. Defaults to gpt-image-2 when configured, otherwise the first enabled image-output model."
                },
                "inputImages": {
                    "type": ["array", "null"],
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Workspace-relative path to an input image for edit/reference use."
                            },
                            "description": {
                                "type": ["string", "null"],
                                "description": "Optional short description of the image's role."
                            }
                        },
                        "required": ["path", "description"]
                    },
                    "description": "Optional input images for edit/reference use."
                },
                "maskPath": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative mask image path for edit mode."
                },
                "size": {
                    "type": ["string", "null"],
                    "description": "Optional output size such as 1024x1024. Defaults to provider/model default."
                },
                "quality": {
                    "type": ["string", "null"],
                    "enum": ["auto", "low", "medium", "high", null],
                    "description": "Optional generation quality. Defaults to auto."
                },
                "background": {
                    "type": ["string", "null"],
                    "enum": ["auto", "opaque", "transparent", null],
                    "description": "Optional background handling. Defaults to auto."
                },
                "outputFormat": {
                    "type": ["string", "null"],
                    "enum": ["png", "jpeg", "webp", null],
                    "description": "Optional saved image format. Defaults to png."
                },
                "compression": {
                    "type": ["integer", "null"],
                    "minimum": 0,
                    "maximum": 100,
                    "description": "Optional compression level from 0 to 100 for supported lossy formats."
                },
                "count": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "maximum": 4,
                    "description": "Optional number of images from 1 to 4. Defaults to 1."
                },
                "outputDir": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative output directory. Defaults to .foco/sessions/<chat_id>/image_gen/."
                },
                "outputName": {
                    "type": ["string", "null"],
                    "description": "Optional output file basename. A sequence suffix is added when generating multiple images."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 300000."
                }
            },
            "required": ["prompt", "mode", "model", "inputImages", "maskPath", "size", "quality", "background", "outputFormat", "compression", "count", "outputDir", "outputName", "timeoutMs"]
        }),
        strict: true,
    }
}

fn write_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: WRITE_FILE_TOOL,
        description: "Write a complete text file, or replace a precise 1-based inclusive line range inside an existing workspace file. Prefer the line-range mode for small single-location edits after reading the target lines.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative file path. Parent directories must already exist."
                },
                "content": {
                    "type": "string",
                    "description": "Complete file content when startLine/endLine are null, or replacement text for the selected line range when both are integers. For line-range writes, include only the replacement lines for that range."
                },
                "startLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based first line to replace, inclusive. Set both startLine and endLine to integers for line-range mode; set both to null for a complete-file write."
                },
                "endLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based last line to replace, inclusive. Set both startLine and endLine to integers for line-range mode; set both to null for a complete-file write."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["path", "content", "startLine", "endLine", "timeoutMs"]
        }),
        strict: true,
    }
}

fn edit_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: EDIT_FILE_TOOL,
        description: "Replace exact text in an existing workspace text file. Before calling edit_file, call read_file for the latest file content and copy oldStr exactly from that current content. By default this tool only edits when oldStr matches exactly once; set replaceAll to true only when every match should be replaced.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative existing file path to edit."
                },
                "oldStr": {
                    "type": "string",
                    "description": "Exact text to replace. It must come from the latest read_file output after removing read_file line-number prefixes."
                },
                "newStr": {
                    "type": "string",
                    "description": "Replacement text."
                },
                "replaceAll": {
                    "type": ["boolean", "null"],
                    "description": "Set true to replace every exact oldStr match. Set false or null to require exactly one match."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["path", "oldStr", "newStr", "replaceAll", "timeoutMs"]
        }),
        strict: true,
    }
}

fn create_todo_graph_definition() -> ToolDefinition {
    ToolDefinition {
        name: CREATE_TODO_GRAPH_TOOL,
        description: "Create or replace the current chat's todo graph. Use this instead of plain todo lists to preserve task context, dependencies, acceptance criteria, summaries, and nested subtasks.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "tasks": {
                    "type": "array",
                    "items": todo_graph_task_schema(),
                    "description": "Top-level tasks for the current chat todo graph."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["tasks", "timeoutMs"]
        }),
        strict: true,
    }
}

fn update_todo_graph_definition() -> ToolDefinition {
    ToolDefinition {
        name: UPDATE_TODO_GRAPH_TOOL,
        description: "Patch one task in the current chat's todo graph without resending the entire graph. Pass the task id and only the fields that should change.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "Id of the task to patch."
                },
                "patch": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "title": {
                            "type": ["string", "null"],
                            "description": "New task title, or null to leave unchanged."
                        },
                        "status": {
                            "type": ["string", "null"],
                            "enum": ["pending", "ready", "running", "blocked", "completed", "failed", "cancelled", null],
                            "description": "New task status, or null to leave unchanged."
                        },
                        "dependsOn": {
                            "type": ["array", "null"],
                            "items": { "type": "string" },
                            "description": "Complete replacement dependency id list, or null to leave unchanged."
                        },
                        "acceptance": {
                            "type": ["array", "null"],
                            "items": { "type": "string" },
                            "description": "Complete replacement acceptance criteria list, or null to leave unchanged."
                        },
                        "summary": {
                            "type": ["string", "null"],
                            "description": "New task progress/context summary, or null to leave unchanged."
                        },
                        "subtasks": {
                            "type": ["array", "null"],
                            "items": todo_graph_task_schema(),
                            "description": "Complete replacement nested subtask list, or null to leave unchanged."
                        }
                    },
                    "required": ["title", "status", "dependsOn", "acceptance", "summary", "subtasks"]
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["taskId", "patch", "timeoutMs"]
        }),
        strict: true,
    }
}

fn get_todo_graph_definition() -> ToolDefinition {
    ToolDefinition {
        name: GET_TODO_GRAPH_TOOL,
        description: "Read the current chat's todo graph, optionally filtering tasks by id or status such as completed, pending, ready, running, blocked, failed, or cancelled.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "status": {
                    "type": ["string", "null"],
                    "enum": ["pending", "ready", "running", "blocked", "completed", "failed", "cancelled", null],
                    "description": "Optional task status filter. Null returns all statuses."
                },
                "taskId": {
                    "type": ["string", "null"],
                    "description": "Optional exact task id filter. Null returns all task ids."
                },
                "includeSubtasks": {
                    "type": "boolean",
                    "description": "When filtering, include matching task subtasks in the returned task objects."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["status", "taskId", "includeSubtasks", "timeoutMs"]
        }),
        strict: true,
    }
}

fn ask_question_definition() -> ToolDefinition {
    ToolDefinition {
        name: ASK_QUESTION_TOOL,
        description: "Ask the user one or more blocking questions through the Foco UI when required information is missing. Provide choices when an answer should be selected from known options; otherwise allow free-form input.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "questions": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "Clear question to show the user."
                            },
                            "options": {
                                "type": ["array", "null"],
                                "items": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Short visible option label."
                                        },
                                        "value": {
                                            "type": "string",
                                            "description": "Exact value returned when the user selects this option."
                                        },
                                        "description": {
                                            "type": ["string", "null"],
                                            "description": "Optional one-sentence explanation of this option."
                                        }
                                    },
                                    "required": ["label", "value", "description"]
                                },
                                "description": "Optional choices for this question. Null means free-form input only."
                            },
                            "allowFreeText": {
                                "type": "boolean",
                                "description": "Whether the user may type an answer manually."
                            }
                        },
                        "required": ["question", "options", "allowFreeText"]
                    },
                    "description": "Questions that must all be answered before the tool returns."
                }
            },
            "required": ["questions"]
        }),
        strict: true,
    }
}

fn run_command_definition() -> ToolDefinition {
    ToolDefinition {
        name: RUN_COMMAND_TOOL,
        description: "Run a local command in the active workspace without invoking a shell.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Executable name or path. Do not include arguments here."
                },
                "args": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Command arguments."
                },
                "cwd": {
                    "type": ["string", "null"],
                    "description": "Optional workspace-relative working directory. Defaults to the workspace root."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional command timeout in milliseconds. Defaults to 60000."
                }
            },
            "required": ["command", "args", "cwd", "timeoutMs"]
        }),
        strict: true,
    }
}

fn sleep_definition() -> ToolDefinition {
    ToolDefinition {
        name: SLEEP_TOOL,
        description: "Pause tool execution for the requested duration.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "durationMs": {
                    "type": "integer",
                    "description": "Pause duration in milliseconds."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 600000."
                }
            },
            "required": ["durationMs", "timeoutMs"]
        }),
        strict: true,
    }
}

fn todo_graph_task_schema() -> Value {
    todo_graph_task_schema_with_depth(3)
}

fn todo_graph_task_schema_with_depth(depth: usize) -> Value {
    let subtasks_schema = if depth == 0 {
        json!({
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": false,
                "properties": {},
                "required": []
            },
            "maxItems": 0
        })
    } else {
        json!({
            "type": "array",
            "items": todo_graph_task_schema_with_depth(depth - 1)
        })
    };

    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": {
                "type": "string",
                "description": "Stable unique task id inside the graph."
            },
            "title": {
                "type": "string",
                "description": "Short human-readable task title."
            },
            "status": {
                "type": "string",
                "enum": ["pending", "ready", "running", "blocked", "completed", "failed", "cancelled"],
                "description": "Task execution status."
            },
            "dependsOn": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Task ids that must be completed or resolved before this task can proceed."
            },
            "acceptance": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Acceptance criteria for this task."
            },
            "summary": {
                "type": "string",
                "description": "Current context, decisions, blockers, and progress summary for interruption recovery."
            },
            "createdAt": {
                "type": ["string", "null"],
                "description": "Ignored on input; the server writes the task creation timestamp."
            },
            "updatedAt": {
                "type": ["string", "null"],
                "description": "Ignored on input; the server writes the task update timestamp."
            },
            "subtasks": subtasks_schema
        },
        "required": ["id", "title", "status", "dependsOn", "acceptance", "summary", "createdAt", "updatedAt", "subtasks"]
    })
}
