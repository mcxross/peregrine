export function absolutePath(rootPath: string, relativePath: string) {
  const root = rootPath.replace(/\/+$/, "");
  const relative = relativePath.replace(/^\/+/, "");

  return `${root}/${relative}`;
}

export function fileUri(rootPath: string, relativePath = "") {
  const path = relativePath ? absolutePath(rootPath, relativePath) : rootPath;
  const normalized = path.replace(/\\/g, "/");

  return `file://${normalized.split("/").map(encodeURIComponent).join("/")}`;
}

export function relativePathFromFileUri(rootPath: string, uri: string) {
  if (!uri.startsWith("file://")) {
    return null;
  }

  const decodedPath = decodeURIComponent(uri.slice("file://".length));
  const normalizedRoot = rootPath.replace(/\\/g, "/").replace(/\/+$/, "");
  const normalizedPath = decodedPath.replace(/\\/g, "/");
  const prefix = `${normalizedRoot}/`;

  if (!normalizedPath.startsWith(prefix)) {
    return null;
  }

  return normalizedPath.slice(prefix.length);
}

