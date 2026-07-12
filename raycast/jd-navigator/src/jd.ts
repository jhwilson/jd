import { homedir } from "node:os";

export type JdNodeType = "Range" | "Category" | "ItemDir" | "File" | "Link" | "Other";

export interface JdLink {
  url: string;
  label?: string;
}

export interface JdNode {
  id: string;
  code: string | null;
  title: string;
  path: string;
  node_type: JdNodeType;
  location: string | null;
  url: string | null;
  locations?: string[];
  links?: JdLink[];
  has_notes: boolean;
  children: JdNode[];
}

export interface JdTree {
  roots: JdNode[];
  warnings?: string[];
}

export interface Row {
  id: string;
  code?: string;
  title: string;
  path: string;
  nodeType: JdNodeType;
  url?: string;
  location?: string;
  locations: string[];
  links: JdLink[];
  hasNotes: boolean;
  breadcrumb: string;
  keywords: string[];
}

export function parseRoots(preference: string): string[] {
  return preference
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((root) => (root === "~" ? homedir() : root.startsWith("~/") ? `${homedir()}${root.slice(1)}` : root));
}

const displayName = (node: JdNode) => (node.code ? `${node.code} ${node.title}` : node.title);

export function flatten(tree: JdTree, showFiles: boolean): Row[] {
  const rows: Row[] = [];

  const visit = (node: JdNode, ancestors: JdNode[]) => {
    const ancestorCodes = ancestors.flatMap((ancestor) => (ancestor.code ? [ancestor.code] : []));
    const ancestorTitleWords = ancestors.flatMap((ancestor) => ancestor.title.split(/\s+/).filter(Boolean));
    const codeKeywords = node.code ? [node.code, node.code.replace(/\./g, "")] : [];

    if (node.node_type !== "File" || showFiles) {
      rows.push({
        id: node.id,
        ...(node.code ? { code: node.code } : {}),
        title: node.title,
        path: node.path,
        nodeType: node.node_type,
        ...(node.url ? { url: node.url } : {}),
        ...(node.location ? { location: node.location } : {}),
        locations: node.locations ?? [],
        links: node.links ?? [],
        hasNotes: node.has_notes,
        breadcrumb: ancestors.map(displayName).join(" > "),
        keywords: [...codeKeywords, ...ancestorCodes, ...ancestorTitleWords],
      });
    }

    for (const child of node.children) visit(child, [...ancestors, node]);
  };

  for (const root of tree.roots) for (const child of root.children) visit(child, [root]);
  return rows;
}
