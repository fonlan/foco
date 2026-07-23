#!/usr/bin/env node

/**
 * Reports native interactive JSX left in production code. This is deliberately
 * a coverage report, not a component detector: semantic CSS tokens never count
 * as a HeroUI component migration.
 *
 * Usage: npm run audit:heroui
 */
import { readdir, readFile } from "node:fs/promises";
import { basename, relative, resolve } from "node:path";
import ts from "typescript";

const workspaceRoot = basename(process.cwd()) === "web"
  ? resolve(process.cwd(), "..")
  : process.cwd();
const root = resolve(workspaceRoot, "web");
const productionDirectoryNames = new Set([".mem", "__tests__", "dist", "node_modules", "test-utils"]);

/**
 * The only native production controls permitted by the migration contract.
 * An entry is valid only at its named file, for its named element, and with the
 * matching data-heroui-exception marker. This prevents a future native control
 * from bypassing the guard simply by copying an exception attribute.
 */
const writtenExceptions = {
  "native-file-input": {
    file: "web/shared/ui/settings-controls.tsx",
    kind: "input",
    inputType: "file",
    target: "browser file input (written exception)",
    reason: "Browser file selection requires an actual input[type=file].",
    owner: "The visible trigger remains a HeroUI Button; the input stays labelled and hidden.",
    removal: "Remove when HeroUI offers an equivalent browser-file capability.",
  },
  "native-form-submit": {
    file: "web/features/chat/ChatPanel.tsx",
    kind: "button",
    target: "native form submit (written exception)",
    reason: "Modifier-aware queueing must preserve the native form submit event and submitter.",
    owner: "The button remains labelled and keeps the existing Ctrl queue and normal submit behavior.",
    removal: "Remove when HeroUI Button can preserve the same native submitter and modifier semantics.",
  },
  "native-plan-drag": {
    file: "web/features/context/ContextPanel.tsx",
    kind: "button",
    target: "native plan drag handle (written exception)",
    reason: "Plan ordering depends on native draggable and DragEvent dataTransfer semantics.",
    owner: "The handle remains labelled and is only used for pointer drag/reorder; selection and actions use HeroUI controls.",
    removal: "Remove when HeroUI Button exposes draggable and typed drag handlers without changing the reorder flow.",
  },
  "native-chat-tab": {
    file: "web/App.tsx",
    kind: "button",
    target: "native composite chat tab (written exception)",
    reason: "The tab must retain its established tab role, title, custom scroll behavior, context menu, and a separate close action.",
    owner: "The selectable tab remains keyboard-addressable; adjacent scrolling and close controls use HeroUI Button.",
    removal: "Remove when HeroUI Tabs supports this composite closable-tab anatomy without nested interactive controls.",
  },
};

const targetFor = (file, kind, inputType, exceptionKind) => {
  const exception = exceptionKind ? writtenExceptions[exceptionKind] : undefined;
  if (exception) {
    const matches = exception.file === file && exception.kind === kind && exception.inputType === inputType;
    if (matches) return { ...exception, status: "exception" };
    return {
      target: `invalid ${exceptionKind} exception`,
      status: "migrate",
    };
  }
  if (kind === "button") return { target: "Button", status: "migrate" };
  if (kind === "textarea") return { target: "TextArea", status: "migrate" };
  if (kind === "select") return { target: "Select + ListBox", status: "migrate" };
  if (kind === "[role=dialog]" || kind === "[aria-modal]" || kind === "dialog") {
    return { target: "Modal", status: "migrate" };
  }

  if (inputType === "file") {
    return {
      target: "documented browser file input exception",
      status: "migrate",
    };
  }

  if (inputType === "checkbox") {
    return { target: "Checkbox or Switch", status: "migrate" };
  }

  if (inputType === "radio") {
    return { target: "RadioGroup + Radio", status: "migrate" };
  }

  return { target: "TextField + Input", status: "migrate" };
};

async function filesIn(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) {
        return productionDirectoryNames.has(entry.name) ? [] : filesIn(path);
      }
      return entry.isFile() && /\.tsx$/.test(entry.name) && !/\.(test|spec)\.tsx$/.test(entry.name)
        ? [path]
        : [];
    }),
  );
  return nested.flat();
}

const rows = [];
const herouiConsumers = [];
for (const file of await filesIn(root)) {
  const source = await readFile(file, "utf8");
  const sourceFile = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const relativeFile = relative(workspaceRoot, file);
  const importedComponents = sourceFile.statements.flatMap((statement) => {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) return [];
    if (!statement.moduleSpecifier.text.endsWith("shared/ui")) return [];
    const bindings = statement.importClause?.namedBindings;
    if (!bindings) return [];
    if (ts.isNamespaceImport(bindings)) return ["*"];
    return bindings.elements.map((specifier) => specifier.name.text);
  });
  if (importedComponents.length) {
    herouiConsumers.push({ file: relativeFile, components: importedComponents.sort() });
  }
  const hasAttribute = (attributes, name) =>
    attributes.properties.some(
      (property) => ts.isJsxAttribute(property) && property.name.text === name,
    );
  const attributeValue = (attributes, name) => {
    const attribute = attributes.properties.find(
      (property) => ts.isJsxAttribute(property) && property.name.text === name,
    );
    return attribute && attribute.initializer && ts.isStringLiteral(attribute.initializer)
      ? attribute.initializer.text
      : undefined;
  };
  const visit = (node) => {
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const element = node.tagName.getText(sourceFile);
      const inputType = element === "input" ? attributeValue(node.attributes, "type")?.toLowerCase() : undefined;
      const exceptionKind = attributeValue(node.attributes, "data-heroui-exception");
      const role = attributeValue(node.attributes, "role");
      const hasAriaModal = hasAttribute(node.attributes, "aria-modal");
      const kind = ["button", "input", "select", "textarea", "dialog"].includes(element)
        ? element
        : role === "dialog"
          ? "[role=dialog]"
          : hasAriaModal
            ? "[aria-modal]"
          : undefined;

      if (kind) {
        const line = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
        rows.push({
          file: relativeFile,
          line,
          kind,
          ...(kind === "input" ? { inputType: inputType ?? "text" } : {}),
          ...targetFor(relativeFile, kind, inputType, exceptionKind),
        });
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
}

const byFile = new Map();
for (const row of rows) {
  const group = byFile.get(row.file) ?? [];
  group.push(row);
  byFile.set(row.file, group);
}

console.log("# HeroUI component coverage audit\n");
console.log(`Native JSX controls: ${rows.length}`);
console.log(`Mapped for migration: ${rows.filter((row) => row.status === "migrate").length}`);
console.log(`Written exceptions: ${rows.filter((row) => row.status === "exception").length}\n`);
console.log("This report is a source-contract guard. CSS semantic-token usage is not component coverage.\n");

console.log("## HeroUI shared-barrel consumers\n");
for (const consumer of herouiConsumers.sort((a, b) => a.file.localeCompare(b.file))) {
  console.log(`- ${consumer.file}: ${consumer.components.join(", ")}`);
}
console.log("");

for (const [file, fileRows] of [...byFile].sort(([a], [b]) => a.localeCompare(b))) {
  const summary = Object.entries(
    Object.groupBy(fileRows, (row) => `${row.kind} → ${row.target}`),
  )
    .map(([label, entries]) => `${label}: ${entries.length}`)
    .join(", ");
  console.log(`## ${file}\n${summary}\n`);
  for (const row of fileRows) {
    const exception = row.status === "exception"
      ? ` — exception: ${row.reason} Accessibility: ${row.owner} Remove: ${row.removal}`
      : "";
    const source = row.kind === "[role=dialog]" || row.kind === "[aria-modal]"
      ? row.kind === "[aria-modal]" ? "[aria-modal]" : "[role=\"dialog\"]"
      : `<${row.kind}${row.inputType ? ` type=\"${row.inputType}\"` : ""}>`;
    console.log(`- L${row.line}: ${source} → ${row.target}${exception}`);
  }
  console.log("");
}

const migrationRows = rows.filter((row) => row.status === "migrate");
if (migrationRows.length) {
  console.error(
    `HeroUI source-contract failed: ${migrationRows.length} native control(s) need migration. ` +
      "Use a shared HeroUI component, or add a narrowly-scoped documented exception to this script.",
  );
  process.exitCode = 1;
}
