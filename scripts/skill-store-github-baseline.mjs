#!/usr/bin/env node

const DEFAULT_SOURCE = "chuspeeism/dashiAI-ppt-skill";
const DEFAULT_REF = "main";
const SKILL_FILE_NAME = "SKILL.md";
const BINARY_EXTENSIONS = new Set([
  ".avif",
  ".gif",
  ".ico",
  ".jpeg",
  ".jpg",
  ".mov",
  ".mp3",
  ".mp4",
  ".otf",
  ".pdf",
  ".png",
  ".ttf",
  ".wav",
  ".webm",
  ".webp",
  ".woff",
  ".woff2",
  ".zip",
]);

const source = process.argv[2] ?? DEFAULT_SOURCE;
const gitRef = process.argv[3] ?? DEFAULT_REF;

if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(source)) {
  throw new Error(`expected GitHub owner/repo, received: ${source}`);
}

const treeUrl = new URL(
  `https://api.github.com/repos/${source}/git/trees/${encodeURIComponent(gitRef)}`,
);
treeUrl.searchParams.set("recursive", "1");

const response = await fetch(treeUrl, {
  headers: {
    Accept: "application/vnd.github+json",
    "User-Agent": "foco-skill-store-baseline",
    "X-GitHub-Api-Version": "2022-11-28",
  },
});
if (!response.ok) {
  throw new Error(`GitHub tree request returned ${response.status}`);
}

const tree = await response.json();
if (!Array.isArray(tree.tree)) {
  throw new Error("GitHub tree response did not include a tree array");
}
if (tree.truncated) {
  throw new Error("GitHub returned a truncated recursive tree");
}

const blobs = tree.tree.filter(
  (item) => item?.type === "blob" && typeof item.path === "string",
);
const skillFiles = blobs.filter(
  (item) => item.path === SKILL_FILE_NAME || item.path.endsWith(`/${SKILL_FILE_NAME}`),
);
if (skillFiles.length !== 1) {
  throw new Error(
    `expected one ${SKILL_FILE_NAME}, found ${skillFiles.length}: ${skillFiles
      .map((item) => item.path)
      .join(", ")}`,
  );
}

const skillPath = skillFiles[0].path;
const skillRoot = skillPath === SKILL_FILE_NAME
  ? ""
  : skillPath.slice(0, -(`/${SKILL_FILE_NAME}`.length));
const skillBlobs = blobs
  .filter((item) => isUnderRoot(item.path, skillRoot))
  .sort((left, right) => left.path.localeCompare(right.path));
const binaryBlobs = skillBlobs.filter((item) =>
  BINARY_EXTENSIONS.has(extension(item.path)),
);
const totalBytes = sumSizes(skillBlobs);
const binaryBytes = sumSizes(binaryBlobs);

const result = {
  source,
  ref: gitRef,
  resolvedTreeishSha: tree.sha,
  treeEntryCount: tree.tree.length,
  repositoryBlobCount: blobs.length,
  skillMdPaths: skillFiles.map((item) => item.path),
  skillRoot,
  skillBlobCount: skillBlobs.length,
  skillTotalBytes: totalBytes,
  currentPreviewRequestShape: {
    treeRequests: 1,
    serialRawFileRequests: skillBlobs.length,
    totalGitHubRequests: skillBlobs.length + 1,
  },
  binaryLike: {
    extensions: [...BINARY_EXTENSIONS].sort(),
    count: binaryBlobs.length,
    bytes: binaryBytes,
  },
  largeFiles: {
    atLeast100KiB: countAtLeast(skillBlobs, 100 * 1024),
    atLeast1MiB: countAtLeast(skillBlobs, 1024 * 1024),
    atLeast2MiB: countAtLeast(skillBlobs, 2 * 1024 * 1024),
  },
  largestFiles: [...skillBlobs]
    .sort((left, right) => size(right) - size(left))
    .slice(0, 10)
    .map((item) => ({ path: item.path, bytes: size(item) })),
};

console.log(JSON.stringify(result, null, 2));

function isUnderRoot(filePath, root) {
  return root === "" || filePath === root || filePath.startsWith(`${root}/`);
}

function extension(filePath) {
  const fileName = filePath.split("/").at(-1) ?? "";
  const dot = fileName.lastIndexOf(".");
  return dot >= 0 ? fileName.slice(dot).toLowerCase() : "";
}

function size(item) {
  return Number.isFinite(item?.size) ? item.size : 0;
}

function sumSizes(items) {
  return items.reduce((total, item) => total + size(item), 0);
}

function countAtLeast(items, threshold) {
  return items.filter((item) => size(item) >= threshold).length;
}
