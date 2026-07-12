import { Action, ActionPanel, closeMainWindow, Icon, showToast, Toast } from "@raycast/api";
import { execFile } from "node:child_process";
import path from "node:path";
import { Row } from "./jd";
import { NotesDetail } from "./notes";

export interface Preferences {
  jdHelperPath: string;
  roots: string;
  primaryAction: "finder" | "ghostty" | "cursor";
  showFiles: boolean;
}

type ActionCallbacks = { onToggleDetail: () => void; onRefresh: () => void };
type Opener = "finder" | "ghostty" | "cursor";

const isDirectory = (row: Row) => row.nodeType === "Range" || row.nodeType === "Category" || row.nodeType === "ItemDir" || row.nodeType === "Other";

function GhosttyAction({ row }: { row: Row }) {
  const dir = isDirectory(row) ? row.path : path.dirname(row.path);
  return (
    <Action
      title="Open in Ghostty"
      icon={Icon.Terminal}
      onAction={async () => {
        try {
          await new Promise<void>((resolve, reject) => execFile("/usr/bin/open", ["-na", "Ghostty", "--args", `--working-directory=${dir}`], (error) => (error ? reject(error) : resolve())));
          await closeMainWindow();
        } catch (error) {
          await showToast({ style: Toast.Style.Failure, title: "Could not open Ghostty", message: error instanceof Error ? error.message : String(error) });
        }
      }}
    />
  );
}

function OpenAction({ opener, row }: { opener: Opener; row: Row }) {
  if (opener === "ghostty") return <GhosttyAction row={row} />;
  if (opener === "cursor") return <Action.Open title="Open in Cursor" target={row.path} application="Cursor" icon={Icon.Code} />;
  return isDirectory(row) ? <Action.Open title="Open in Finder" target={row.path} application="Finder" /> : <Action.ShowInFinder path={row.path} />;
}

export function buildActions(row: Row, prefs: Preferences, { onToggleDetail, onRefresh }: ActionCallbacks) {
  const openers: Opener[] = [prefs.primaryAction, ...(["finder", "ghostty", "cursor"] as Opener[]).filter((opener) => opener !== prefs.primaryAction)];

  return (
    <ActionPanel>
      {row.nodeType === "Link" && row.url ? <Action.OpenInBrowser url={row.url} /> : null}
      {openers.map((opener) => <OpenAction key={opener} opener={opener} row={row} />)}
      {row.nodeType === "File" ? <Action.Open title="Open (Default App)" target={row.path} /> : null}
      {row.nodeType === "File" ? <Action.OpenWith path={row.path} /> : null}
      {row.links.length > 0 ? <ActionPanel.Section title="Links">{row.links.map((link) => <Action.OpenInBrowser key={link.url} title={link.label ?? link.url} url={link.url} />)}</ActionPanel.Section> : null}
      <ActionPanel.Section title="Utilities">
        <Action.CopyToClipboard title="Copy Path" content={row.path} shortcut={{ modifiers: ["cmd", "shift"], key: "c" }} />
        {row.code ? <Action.CopyToClipboard title="Copy JD Code" content={row.code} shortcut={{ modifiers: ["cmd", "shift"], key: "." }} /> : null}
        {row.hasNotes ? <Action.Push title="View Notes" target={<NotesDetail dir={row.path} />} /> : null}
        <Action title="Toggle Details" shortcut={{ modifiers: ["cmd"], key: "d" }} onAction={onToggleDetail} />
        <Action title="Refresh" shortcut={{ modifiers: ["cmd"], key: "r" }} onAction={onRefresh} />
      </ActionPanel.Section>
    </ActionPanel>
  );
}
