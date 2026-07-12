import { Action, ActionPanel, Icon, List, Toast, getPreferenceValues, openExtensionPreferences, showToast } from "@raycast/api";
import { useExec, usePromise } from "@raycast/utils";
import { existsSync, promises as fs } from "node:fs";
import path from "node:path";
import { useMemo, useRef, useState } from "react";
import { buildActions, Preferences } from "./actions";
import { JdNodeType, JdTree, Row, flatten, parseRoots } from "./jd";

const iconFor = (nodeType: JdNodeType) => ({ Range: Icon.Folder, ItemDir: Icon.Folder, Category: Icon.Tray, File: Icon.Document, Link: Icon.Link, Other: Icon.QuestionMark })[nodeType];
const titleFor = (row: Row) => (row.code ? `${row.code} ${row.title}` : row.title);

export default function SearchCommand() {
  const prefs = getPreferenceValues<Preferences>();
  const [showDetail, setShowDetail] = useState(false);
  const [selectedId, setSelectedId] = useState<string>();
  const lastToastedWarnings = useRef("");
  const helperExists = existsSync(prefs.jdHelperPath);
  const { data, error, isLoading, revalidate } = useExec<JdTree>(prefs.jdHelperPath, ["scan", ...parseRoots(prefs.roots)], {
    execute: helperExists,
    timeout: 120_000,
    keepPreviousData: true,
    parseOutput: ({ stdout }) => JSON.parse(stdout) as JdTree,
    failureToastOptions: { title: "jd-helper scan failed" },
    onData: (tree) => {
      const warnings = JSON.stringify(tree.warnings ?? []);
      if (tree.warnings?.length && warnings !== lastToastedWarnings.current) {
        lastToastedWarnings.current = warnings;
        void showToast({ style: Toast.Style.Failure, title: "jd-helper scan warnings", message: tree.warnings.join("\n") });
      }
    },
  });
  const rows = useMemo(() => (data ? flatten(data, prefs.showFiles) : []), [data, prefs.showFiles]);
  const selectedRow = rows.find((row) => row.id === selectedId) ?? rows[0];
  const { data: noteMarkdown } = usePromise(
    async (row?: Row) => (row?.hasNotes ? fs.readFile(path.join(row.path, ".jdmeta.md"), "utf8") : undefined),
    [selectedRow],
  );

  if (!helperExists) {
    return <List><List.EmptyView title="jd-helper not found" description="Build it with cargo build --release in /Users/justin/bin/jd." actions={<ActionPanelForPreferences />} /></List>;
  }

  if (error) {
    return <List><List.EmptyView title="jd-helper scan failed" description={error.message} /></List>;
  }

  // Built-in filtering + per-item keywords is instant at this tree's scale; if it
  // ever lags, switch to filtering={false} + onSearchTextChange with a memoized filter.
  return (
    <List isLoading={isLoading} isShowingDetail={showDetail} onSelectionChange={(id) => setSelectedId(id ?? undefined)} filtering>
      {rows.map((row) => (
        <List.Item
          key={row.id}
          id={row.id}
          title={titleFor(row)}
          subtitle={showDetail ? undefined : row.breadcrumb}
          icon={iconFor(row.nodeType)}
          keywords={row.keywords}
          accessories={showDetail ? [] : [
            ...(row.hasNotes ? [{ icon: Icon.Snippets, tooltip: "Has notes" }] : []),
            ...(row.links.length ? [{ text: String(row.links.length), icon: Icon.Link, tooltip: `${row.links.length} link${row.links.length === 1 ? "" : "s"}` }] : []),
          ]}
          detail={<List.Item.Detail markdown={row.id === selectedRow?.id && row.hasNotes ? noteMarkdown : undefined} metadata={<Metadata row={row} />} />}
          actions={buildActions(row, prefs, { onToggleDetail: () => setShowDetail((value) => !value), onRefresh: revalidate })}
        />
      ))}
    </List>
  );
}

function ActionPanelForPreferences() {
  return <ActionPanel><Action title="Open Extension Preferences" onAction={openExtensionPreferences} /></ActionPanel>;
}

function Metadata({ row }: { row: Row }) {
  return (
    <List.Item.Detail.Metadata>
      {row.code ? <List.Item.Detail.Metadata.Label title="Code" text={row.code} /> : null}
      <List.Item.Detail.Metadata.Label title="Type" text={row.nodeType} />
      <List.Item.Detail.Metadata.Label title="Path" text={row.path} />
      {row.location ? <List.Item.Detail.Metadata.Label title="Location" text={row.location} /> : null}
      {row.locations.map((location) => <List.Item.Detail.Metadata.Label key={location} title="Location" text={location} />)}
      {row.links.map((link) => <List.Item.Detail.Metadata.Link key={link.url} title={link.label ?? link.url} target={link.url} text={link.url} />)}
    </List.Item.Detail.Metadata>
  );
}
