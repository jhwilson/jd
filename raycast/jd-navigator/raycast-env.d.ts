/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** jd-helper Binary - Path to the jd-helper binary */
  "jdHelperPath": string,
  /** JD Roots - Root directories passed to jd-helper scan (space-separated) */
  "roots": string,
  /** Primary Action (Enter) - Which open action Enter triggers on directories */
  "primaryAction": "finder" | "ghostty" | "cursor",
  /** Results - Show file entries in search results */
  "showFiles": boolean
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `search` command */
  export type Search = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `search` command */
  export type Search = {}
}

