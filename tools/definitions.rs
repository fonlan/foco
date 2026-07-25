use serde_json::{Value, json};

use crate::{
    APPLY_PATCH_TOOL, ASK_QUESTION_TOOL, CREATE_PLAN_TOOL, CREATE_TODO_GRAPH_TOOL,
    DELETE_PLAN_TOOL, EDIT_FILE_TOOL, FIND_FILES_TOOL, GET_COMMAND_OUTPUT_TOOL, GET_PLANS_TOOL,
    GET_TODO_GRAPH_TOOL, GRAPH_EXPLORE_TOOL, GRAPH_FIND_CALLEES_TOOL, GRAPH_FIND_CALLERS_TOOL,
    GRAPH_FIND_CHILDREN_TOOL, GRAPH_FIND_IMPORTERS_TOOL, GRAPH_FIND_IMPORTS_TOOL,
    GRAPH_FIND_REFERENCES_TOOL, GRAPH_FIND_SYMBOLS_TOOL, GRAPH_RELATED_FILES_TOOL, IMAGE_GEN_TOOL,
    READ_FILE_TOOL, READ_SPEC_TOOL, RUN_COMMAND_TOOL, SEARCH_TEXT_TOOL, SLEEP_TOOL,
    STOP_COMMAND_TOOL, ToolDefinition, UPDATE_PLAN_STEP_TOOL, UPDATE_PLAN_TOOL, UPDATE_SPEC_TOOL,
    UPDATE_TODO_GRAPH_TOOL, WEB_FETCH_TOOL, WEB_SEARCH_TOOL, WRITE_FILE_TOOL,
};

pub(crate) fn builtin_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_file_definition(),
        find_files_definition(),
        graph_find_symbols_definition(),
        graph_find_callers_definition(),
        graph_find_callees_definition(),
        graph_find_children_definition(),
        graph_find_references_definition(),
        graph_find_imports_definition(),
        graph_find_importers_definition(),
        graph_related_files_definition(),
        graph_explore_definition(),
        search_text_definition(),
        web_search_definition(),
        web_fetch_definition(),
        image_gen_definition(),
        write_file_definition(),
        edit_file_definition(),
        apply_patch_definition(),
        create_todo_graph_definition(),
        update_todo_graph_definition(),
        get_todo_graph_definition(),
        create_plan_definition(),
        get_plans_definition(),
        update_plan_definition(),
        update_plan_step_definition(),
        delete_plan_definition(),
        read_spec_definition(),
        update_spec_definition(),
        ask_question_definition(),
        run_command_definition(),
        get_command_output_definition(),
        stop_command_definition(),
        sleep_definition(),
    ]
}

fn read_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: READ_FILE_TOOL,
        description: "Read a text file inside the active workspace, or outside the workspace after explicit user authorization, optionally restricted to a 1-based inclusive line range. Ordinary files use the shared soft output budget (~50KiB or 2,000 numbered lines) and the ~128KiB complete ToolExecution/envelope hard limit. When ordinary content exceeds the soft budget, the tool succeeds (is_error=false) with an explicit complete-line prefix: truncated=true, nextStartLine for continuation from the original file, returnedLines/lastReturnedLine, and a model-facing note. This is explicit truncated success, not hidden data loss; continue with startLine=nextStartLine and a non-null inclusive endLine rather than stitching silent mid-line cuts. UTF-8 characters and line contents are never split. If a single complete line cannot fit under the hard envelope without splitting, the tool returns a recoverable error. A single line that only exceeds the soft limit but fits the hard limit is returned in full: when more content remains it is marked truncated=true with a continuable nextStartLine; when that line is the entire content it is marked softBudgetExceeded=true with truncated=false and no nextStartLine (never invent a past-EOF continuation). Ordinary sources may be up to about 32MiB; full unscoped reads of large files return the first complete-line prefix under the soft budget (no separate 128KiB source refusal). Sources above the 32MiB ordinary text protection fail clearly. Files named SKILL.md are an integrity exception: startLine/endLine must both be null, the full document is returned when it is at most 64KiB (SKILL.md does not use silent/complete-line truncation), and oversized SKILL.md files fail outright (partial range reads cannot reconstruct a disabled skill). Non-SKILL.md files under skill directories (references/, scripts, assets) keep normal ranged-read rules. The returned content is prefixed with real 1-based file line numbers for edit targeting; line-number prefixes are not file content and must not be copied into write_file content or edit_file oldStr/newStr values.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative file path, or an absolute path. Absolute paths whose canonical target is inside the current execution workspace are ordinary internal reads (no external authorization). Absolute paths outside the execution workspace require user confirmation or another external-read grant (Skill roots, current-chat attachment allowlist, shared-workspace trust from an isolated worktree, chat allow-all). Only read_file, find_files, and search_text support authorized external paths; write_file, edit_file, run_command, and graph tools do not accept absolute paths outside the execution root."
                },
                "startLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based first line to read. Must be null when endLine is null (full-file mode). Must be null for SKILL.md (full document only). After a truncated ordinary read, continue with startLine=nextStartLine from the previous result together with an inclusive endLine (both must be non-null integers; omitting endLine is invalid when startLine is set). Prefer a finite endLine so the request stays under the soft ~50KiB / 2,000-line budget and the ~128KiB hard cap; if the range is still large the tool may return truncated=true with another nextStartLine."
                },
                "endLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based last line to read, inclusive. Values beyond the file length read through the final line. Must be null when startLine is null. Must be null for SKILL.md (full document only). Continuation after truncated=true requires startLine=nextStartLine and a non-null inclusive endLine together; do not pass startLine with endLine=null. Prefer a finite range under the soft ~50KiB / 2,000-line budget; oversized ranges may still return truncated=true with another nextStartLine."
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
        description: "Find files and directories under a directory using optional glob include/exclude patterns. Results are sorted by path. Responses keep whole entry records under the shared soft output budget (50 KiB or 2,000 lines); when truncated, refine include/exclude or path rather than expecting silent mid-record cuts. Workspace-relative and execution-workspace absolute directories are internal; paths outside the execution workspace require the same user-confirmed external-read grant as read_file. Internal entry paths stay workspace-relative; external entry paths are absolute so they can be passed to read_file.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to search recursively. Use . for the execution workspace root. Accepts workspace-relative paths, absolute paths inside the execution workspace (no external authorization), or absolute paths outside the execution workspace after user confirmation / external-read grant. Graph, write_file, edit_file, and run_command still do not accept absolute paths outside the execution root. Include/exclude globs match paths relative to the search root."
                },
                "include": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Optional glob patterns matched against paths relative to the search root. Null or an empty array includes everything not excluded. Internal results report workspace-relative paths; external results report absolute paths."
                },
                "exclude": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Optional glob patterns matched against paths relative to the search root to omit."
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
        description: "Find indexed code graph symbol candidates and symbolIds by name, signature, or documentation. Use this for disambiguation or candidate lists; use graph_explore instead when you need source code snippets. Results keep whole symbol records under the shared soft output budget; lower limit when truncated.",
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
        description: "Find static call-site approximations that call the requested symbol. This is not runtime tracing; relationship metadata includes edge kind and provenance. Use graph_explore for source context. Use symbolId from graph_find_symbols when names are ambiguous.",
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
        description: "Find static call-site approximations invoked by the requested symbol. This is not runtime tracing; relationship metadata includes edge kind and provenance. Use graph_explore for source context. Use symbolId from graph_find_symbols when names are ambiguous.",
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

fn graph_find_children_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_CHILDREN_TOOL,
        description: "List direct indexed members of a class, module, impl, trait, or other container symbol. This is one level only and does not recursively expand descendants.",
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
                "kind": {
                    "type": ["string", "null"],
                    "description": "Optional direct-member kind filter, such as method, function, variable, or type_alias."
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
            "required": ["symbolId", "symbol", "path", "kind", "limit", "timeoutMs"]
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

fn graph_find_imports_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_IMPORTS_TOOL,
        description: "List import declarations from one indexed workspace file with module-resolution status, target path/symbol when exact, candidates, and resolver provenance. Set resolved=true to keep only exact results; false returns candidate, unresolved, and external imports.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative indexed file path."
                },
                "resolved": {
                    "type": ["boolean", "null"],
                    "description": "Optional exact-resolution filter. true returns exact imports only; false excludes exact imports."
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
            "required": ["path", "resolved", "limit", "timeoutMs"]
        }),
        strict: true,
    }
}

fn graph_find_importers_definition() -> ToolDefinition {
    ToolDefinition {
        name: GRAPH_FIND_IMPORTERS_TOOL,
        description: "Find workspace files that exactly resolve an import to the requested path. Candidate and unresolved module strings are intentionally excluded, so this is reliable reverse dependency navigation.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative indexed file or module path."
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
        description: "Default code graph tool for source context: find indexed code graph symbols and return matching source snippets with real 1-based line numbers. Use this instead of graph_find_symbols plus read_file when you need code for a symbol or likely target. limit controls only the first preview. When results are truncated by limit or the shared soft output budget, the response provides totalCount, returnedCount, nextOffset (the next snippet index), fullResultPath, and nextStartLine. The complete workspace-local plain-text snapshot is written under `.foco/graph-results/`; continue with read_file(path=fullResultPath, startLine=nextStartLine, endLine=<finite range>). Do not lower limit to recover omitted snippets. If complete collection reaches its safety limit, refine the query instead.",
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
        description: "Search text under a path with ripgrep/rg and return matching lines. The query uses rg pattern syntax. Large result sets return a stable small preview under the shared soft limits (50 KiB or 2,000 lines), with totalMatches/returnedMatches, an opaque continuation token (snapshot id + next offset), and fullResultPath under the execution workspace .foco/search-results/ (never under an external search root). Pass the same query/path plus a non-empty continuation to page further without re-running the search; expired/pruned/mismatched continuations fail with a stable invalid/expired error. Missing, null, empty, or whitespace-only continuation always starts a fresh search. Complete dumps still require ranged read_file when large. When ripgrep output hits the command collection ceiling the tool fails with incomplete/refine guidance rather than inventing a full total. Paths outside the execution workspace require the same user-confirmed external-read grant as read_file; internal match.path stays workspace-relative and external match.path is absolute for read_file reuse.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Ripgrep search pattern. Must match the original query when using continuation."
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search. Use . for the execution workspace root. Accepts workspace-relative paths, absolute paths inside the execution workspace (no external authorization), or absolute paths outside the execution workspace after user confirmation / external-read grant. Must match the original path when using continuation. Graph, write_file, edit_file, and run_command still do not accept absolute paths outside the execution root. Snapshots always write under the execution workspace .foco/search-results/."
                },
                "continuation": {
                    "type": ["string", "null"],
                    "description": "Opaque token from a previous search_text response (format snapshotId:nextOffset). A non-empty token (after trim) pages the existing snapshot instead of running a new search. Null, empty string, whitespace-only, or a missing field at runtime all start a fresh search. Required in the schema for strict-tool compatibility; pass null for the first page."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["query", "path", "continuation", "timeoutMs"]
        }),
        strict: true,
    }
}

fn web_search_definition() -> ToolDefinition {
    ToolDefinition {
        name: WEB_SEARCH_TOOL,
        description: "Search the web for current or external information using the search API configured in Foco settings. Use web_fetch on result URLs when page details or direct source text are needed. Large result payloads use the shared soft output budget (~50KiB / 2,000 lines) and ~128KiB complete envelope hard limit: when over the soft budget the tool succeeds (is_error=false) with an explicit complete-line prefix (truncated=true, nextStartLine, note) and writes the full credential-free readable result under the tool execution workspace `.foco/web-results/` as fullResultPath (local workspace or SSH sidecar workspace via broker file transfer so a later read_file can open it). Field names truncated, nextStartLine, fullResultPath, and note are identical for local and SSH. This is explicit truncated success, not hidden data loss; continue via nextStartLine on the cached file or read_file ranges on fullResultPath. UTF-8 characters and lines are never split. A single line over the hard envelope without a safe complete-line return is a recoverable error; a single line over soft but under hard is returned fully with truncated when more content remains (nextStartLine must be continuable; never invent a past-EOF line).",
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
        description: "Fetch an HTTP or HTTPS URL and return readable text content with basic page metadata. Large pages use the shared soft output budget (~50KiB / 2,000 lines) and ~128KiB complete envelope hard limit: when over the soft budget the tool succeeds (is_error=false) with an explicit complete-line prefix of the readable text (truncated=true, nextStartLine, note) and saves the full credential-free readable result under the tool execution workspace `.foco/web-results/` as fullResultPath (local sessions write the local workspace; SSH sessions transfer via broker into the sidecar workspace so fullResultPath is readable by a later read_file). Prefer continuing with nextStartLine or read_file on fullResultPath rather than assuming mid-line cuts. Optional startLine/endLine still select a 1-based inclusive slice of the readable text before the shared complete-line soft cap applies. Field names truncated, nextStartLine, fullResultPath, and note are identical for local and SSH paths. A single complete line that cannot fit the hard envelope is a recoverable error; a single line over soft but under hard is returned fully with truncated when more remains, or softBudgetExceeded=true with truncated=false and no nextStartLine when that line is the entire content. SKILL.md integrity rules do not apply to web tools.",
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
                    "description": "Optional 1-based first readable-text line to return. Must be set together with endLine; null requests the full page (still subject to complete-line soft truncation with truncated/nextStartLine/fullResultPath). After a truncated fetch, continue from nextStartLine or read_file fullResultPath."
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

fn apply_patch_definition() -> ToolDefinition {
    ToolDefinition {
        name: APPLY_PATCH_TOOL,
        description: "Apply a Codex-compatible *** Begin Patch document inside the execution workspace. Supports Add File, Delete File, Update File, Move to, multi-file patches, and fuzzy context matching. The patch is applied in hunk order; if a later hunk fails, earlier successful edits remain. Only use this when the current runtime has explicitly made apply_patch available.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "A non-empty Codex patch document, beginning with *** Begin Patch and ending with *** End Patch."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["patch", "timeoutMs"]
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

fn create_plan_definition() -> ToolDefinition {
    ToolDefinition {
        name: CREATE_PLAN_TOOL,
        description: "Create a durable workspace plan for the Plan panel. Use workspace-wide unique ids such as plan-<topic>-<timestamp>, plan-phase-<topic>-<timestamp>-*, and plan-step-<topic>-<timestamp>-*; phases are ordered as provided and steps are checkable.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Stable workspace-wide unique plan id. Use a plan-* prefix with a task-specific suffix."
                },
                "title": {
                    "type": "string",
                    "description": "Short plan title shown in the Plan panel."
                },
                "overview": {
                    "type": "string",
                    "description": "Overview describing the plan goal, scope, and important constraints."
                },
                "status": {
                    "type": ["string", "null"],
                    "enum": ["draft", "ready", null],
                    "description": "Initial plan status. Null defaults to ready."
                },
                "sourceChatId": {
                    "type": ["string", "null"],
                    "description": "Optional chat id that produced this plan. Null uses the current chat when available."
                },
                "phases": {
                    "type": "array",
                    "items": plan_phase_schema(),
                    "description": "Ordered implementation phases. Each phase is intended to be implemented in its own session."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["id", "title", "overview", "status", "sourceChatId", "phases", "timeoutMs"]
        }),
        strict: true,
    }
}

fn get_plans_definition() -> ToolDefinition {
    ToolDefinition {
        name: GET_PLANS_TOOL,
        description: "Read workspace plans from the Plan panel store. Use view active for the right panel and view all for history. When offset is non-null, it takes precedence over page for this read; continue only with the returned nextOffset while keeping view and status unchanged. Offset is not a database index or stable snapshot cursor.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "view": {
                    "type": ["string", "null"],
                    "enum": ["active", "all", null],
                    "description": "Plan view. active excludes only completed plans; all includes history. Null defaults to active."
                },
                "status": {
                    "type": ["string", "null"],
                    "enum": ["draft", "ready", "running", "paused", "implemented", "completed", "failed", "cancelled", null],
                    "description": "Optional status filter. Null returns all statuses allowed by view."
                },
                "page": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based page number. Null defaults to 1."
                },
                "pageSize": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "maximum": 10,
                    "description": "Optional page size from 1 to 10. Null defaults to 10."
                },
                "limit": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "maximum": 10,
                    "description": "Optional alias for pageSize from 1 to 10, useful for active view. Null defaults to 10."
                },
                "offset": {
                    "type": ["integer", "null"],
                    "minimum": 0,
                    "description": "Optional zero-based continuation offset returned as nextOffset. When non-null, it takes precedence over page; repeat the same view and status."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["view", "status", "page", "pageSize", "limit", "offset", "timeoutMs"]
        }),
        strict: true,
    }
}

fn update_plan_definition() -> ToolDefinition {
    ToolDefinition {
        name: UPDATE_PLAN_TOOL,
        description: "Patch a durable workspace plan's title, overview, status, or error message without rewriting phases or steps.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "planId": {
                    "type": "string",
                    "description": "Plan id to patch."
                },
                "title": {
                    "type": ["string", "null"],
                    "description": "New plan title, or null to leave unchanged."
                },
                "overview": {
                    "type": ["string", "null"],
                    "description": "New plan overview, or null to leave unchanged."
                },
                "status": {
                    "type": ["string", "null"],
                    "enum": ["draft", "ready", "running", "paused", "implemented", "failed", "cancelled", null],
                    "description": "New plan status, or null to leave unchanged. Use mark_complete action outside this tool for completed."
                },
                "errorMessage": {
                    "type": ["string", "null"],
                    "description": "Error message to store; an empty string clears it, null leaves it unchanged."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["planId", "title", "overview", "status", "errorMessage", "timeoutMs"]
        }),
        strict: true,
    }
}

fn update_plan_step_definition() -> ToolDefinition {
    ToolDefinition {
        name: UPDATE_PLAN_STEP_TOOL,
        description: "Patch one checkable step in a durable workspace plan. Completing all steps makes the plan implemented, not completed; users manually mark completed in the Plan panel.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "planId": {
                    "type": "string",
                    "description": "Plan id containing the step."
                },
                "stepId": {
                    "type": "string",
                    "description": "Step id to patch."
                },
                "title": {
                    "type": ["string", "null"],
                    "description": "New step title, or null to leave unchanged."
                },
                "detail": {
                    "type": ["string", "null"],
                    "description": "New step detail, or null to leave unchanged."
                },
                "acceptance": {
                    "type": ["array", "null"],
                    "items": { "type": "string" },
                    "description": "Complete replacement acceptance criteria, or null to leave unchanged."
                },
                "status": {
                    "type": ["string", "null"],
                    "enum": ["pending", "running", "completed", "failed", "cancelled", null],
                    "description": "New step status, or null to leave unchanged."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["planId", "stepId", "title", "detail", "acceptance", "status", "timeoutMs"]
        }),
        strict: true,
    }
}

fn delete_plan_definition() -> ToolDefinition {
    ToolDefinition {
        name: DELETE_PLAN_TOOL,
        description: "Delete a durable workspace plan created by the current chat. This tool cannot delete plans from other chats.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "planId": {
                    "type": "string",
                    "description": "Plan id to delete."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["planId", "timeoutMs"]
        }),
        strict: true,
    }
}

fn read_spec_definition() -> ToolDefinition {
    ToolDefinition {
        name: READ_SPEC_TOOL,
        description: "Read the Project Spec for the active workspace. The spec is durable workspace context for product, architecture, runtime, data, UI, tool, and operational facts; it is not for temporary todos, logs, secrets, or personal preferences. Large specs use the shared soft output budget (~50KiB / 2,000 lines) and ~128KiB complete envelope hard limit: when over the soft budget the tool succeeds (is_error=false) with an explicit complete-line prefix of contentMarkdown (truncated=true, nextStartLine, returnedLines/lastReturnedLine, totalLines/totalBytes, note). Continue with startLine=nextStartLine and expectedRevision set to the revision from the first page so the multi-page read is pinned to one snapshot; if the revision changed, restart from the first page without startLine. First page: startLine and expectedRevision may both be null. A single complete line over soft but under hard is returned fully with softBudgetExceeded=true and truncated=false (no fake nextStartLine past EOF). UTF-8 characters and Markdown lines are never split.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "startLine": {
                    "type": ["integer", "null"],
                    "description": "Optional 1-based first Markdown line to return. Null starts at line 1 (first page). After truncated=true, continue with startLine=nextStartLine from the previous result together with expectedRevision equal to that page's revision."
                },
                "expectedRevision": {
                    "type": ["integer", "null"],
                    "description": "Required when startLine is non-null: must match the workspace Spec revision from the first page of this multi-page read. On mismatch the tool returns a recoverable revision conflict and you must restart without startLine. Optional on the first page to pin a known revision."
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["startLine", "expectedRevision", "timeoutMs"]
        }),
        strict: true,
    }
}

fn update_spec_definition() -> ToolDefinition {
    ToolDefinition {
        name: UPDATE_SPEC_TOOL,
        description: "Update the Project Spec for the active workspace using expectedRevision optimistic locking. Call read_spec first and base the update on its latest revision and exact content (use read_spec continuation when the body is truncated). Prefer edits for precise patches; use contentMarkdown only when initializing the spec or when a complete rewrite is genuinely required. Provide exactly one non-null update payload. The spec is durable workspace context; do not use it for temporary todos, logs, secrets, personal preferences, or chat-only notes. Retry from the latest read_spec result if the update conflicts. On success, small results include contentMarkdown; large successful results may set contentOmitted=true and omit the body while still returning revision, updateMode, editCount, and line counts—do not retry the same write; call read_spec instead.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "expectedRevision": {
                    "type": "integer",
                    "description": "Revision returned by the latest read_spec call. The update fails if the stored revision changed."
                },
                "contentMarkdown": {
                    "type": ["string", "null"],
                    "description": "Complete replacement Project Spec markdown content, or null when using edits. Use only for initialization or a necessary complete rewrite. Existing workspace spec size validation applies."
                },
                "edits": {
                    "type": ["array", "null"],
                    "description": "Ordered exact-text patches, or null when using contentMarkdown. Each oldText must be non-empty and match the latest spec exactly once.",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "oldText": {
                                "type": "string",
                                "description": "Exact text from the latest read_spec content. It must be non-empty and match exactly once when this edit is applied."
                            },
                            "newText": {
                                "type": "string",
                                "description": "Replacement text for the unique oldText match."
                            }
                        },
                        "required": ["oldText", "newText"]
                    }
                },
                "timeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional tool timeout in milliseconds. Defaults to 10000."
                }
            },
            "required": ["expectedRevision", "contentMarkdown", "edits", "timeoutMs"]
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
        description: "Run a local command in the active workspace without invoking a shell. Recursive scans must stay inside the workspace. Set background=true to start a managed background process and receive a stable processId immediately. Keep that processId and every returned nextCursor as structured state: call get_command_output with cursor=nextCursor to read only new retained stdout/stderr (use waitMs for a bounded long-poll), and call stop_command to terminate the complete process tree when the process is no longer needed. Background output is bounded and retained only in memory; outputTruncated only means its ring buffer evicted older output, not ordinary response pagination. Do not expect it to survive an application or sidecar restart.",
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
                    "description": "Optional command timeout in milliseconds for foreground execution. Defaults to 60000."
                },
                "background": {
                    "type": ["boolean", "null"],
                    "description": "When true, start a managed background process and return its processId without waiting for completion. Null or false keeps foreground behavior."
                },
                "backgroundTimeoutMs": {
                    "type": ["integer", "null"],
                    "description": "Optional maximum lifetime for a managed background process in milliseconds. Null leaves its lifetime unbounded."
                }
            },
            "required": ["command", "args", "cwd", "timeoutMs", "background", "backgroundTimeoutMs"]
        }),
        strict: true,
    }
}

fn get_command_output_definition() -> ToolDefinition {
    ToolDefinition {
        name: GET_COMMAND_OUTPUT_TOOL,
        description: "Read retained incremental stdout and stderr for a managed background command. Reuse nextCursor as cursor for the next non-consuming read so prior logs are not repeated in context; null starts at the earliest retained output. Large retained output is returned as complete-chunk pages: when hasMore=true and truncated=true, immediately call this tool again with the same processId and cursor=nextCursor. The note confirms this is an explicit successful response pagination, not silent loss. outputTruncated means the process ring buffer evicted older output; cursorExpired means the requested cursor predates retained output. waitMs is a bounded long-poll that returns early when output arrives or the process exits. Stop long-running processes explicitly with stop_command when they are no longer needed.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "processId": { "type": "string", "description": "Stable process handle returned by run_command with background true." },
                "cursor": { "type": ["integer", "null"], "description": "Previous nextCursor. Null reads from the earliest retained output." },
                "waitMs": { "type": ["integer", "null"], "description": "Optional bounded long-poll duration. Returns early when output arrives or the process exits." },
                "timeoutMs": { "type": ["integer", "null"], "description": "Optional tool deadline in milliseconds. Defaults to 10000." }
            },
            "required": ["processId", "cursor", "waitMs", "timeoutMs"]
        }),
        strict: true,
    }
}

fn stop_command_definition() -> ToolDefinition {
    ToolDefinition {
        name: STOP_COMMAND_TOOL,
        description: "Synchronously terminate a managed background command process tree. This tool succeeds only after the process tree has exited and stdout/stderr pipes have been drained; its successful result is always terminal, never running. timeoutMs is the maximum wait budget and a timeout is a tool error. Retained output remains readable afterwards through get_command_output until the bounded in-memory record is cleaned up.",
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "processId": { "type": "string", "description": "Stable process handle returned by run_command with background true." },
                "timeoutMs": { "type": ["integer", "null"], "description": "Optional tool deadline in milliseconds. Defaults to 10000." }
            },
            "required": ["processId", "timeoutMs"]
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

fn plan_phase_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": {
                "type": "string",
                "description": "Stable workspace-wide unique phase id. Use a plan-phase-* prefix with the plan topic and a unique suffix."
            },
            "title": {
                "type": "string",
                "description": "Phase title."
            },
            "summary": {
                "type": ["string", "null"],
                "description": "Implementation summary and boundaries for this phase. Null stores an empty summary."
            },
            "steps": {
                "type": "array",
                "items": plan_step_schema(),
                "description": "Ordered checkable implementation steps for this phase."
            }
        },
        "required": ["id", "title", "summary", "steps"]
    })
}

fn plan_step_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": {
                "type": "string",
                "description": "Stable workspace-wide unique step id. Use a plan-step-* prefix with the plan topic and a unique suffix; do not reuse generic ids like plan-step-tests across plans."
            },
            "title": {
                "type": "string",
                "description": "Short checkable step title."
            },
            "detail": {
                "type": ["string", "null"],
                "description": "Concrete implementation detail. Null stores an empty detail."
            },
            "acceptance": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Acceptance checks for this step."
            }
        },
        "required": ["id", "title", "detail", "acceptance"]
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_plan_schema_requires_every_property_for_strict_providers() {
        let definition = create_plan_definition();
        let required = definition.input_schema["required"]
            .as_array()
            .expect("required array")
            .iter()
            .map(|value| value.as_str().expect("required string"))
            .collect::<Vec<_>>();

        assert_eq!(
            required,
            vec![
                "id",
                "title",
                "overview",
                "status",
                "sourceChatId",
                "phases",
                "timeoutMs"
            ]
        );
        assert_eq!(
            definition.input_schema["additionalProperties"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(
            definition.input_schema["properties"]["status"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(
            definition.input_schema["properties"]["sourceChatId"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(
            definition.input_schema["properties"]["timeoutMs"]["type"],
            json!(["integer", "null"])
        );
    }
}
