import { Detail } from "@raycast/api";
import { usePromise } from "@raycast/utils";
import { promises as fs } from "node:fs";
import path from "node:path";

export function NotesDetail({ dir }: { dir: string }) {
  const { data, error, isLoading } = usePromise(async (directory: string) => fs.readFile(path.join(directory, ".jdmeta.md"), "utf8"), [dir]);
  const markdown = error ? "# Unable to read notes\n\nThe folder note could not be read." : (data ?? "");

  return <Detail markdown={markdown} isLoading={isLoading} navigationTitle="Notes" />;
}
