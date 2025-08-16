# jd zsh wrapper function
# Source this file from ~/.zshrc or install via install.sh
#
# Keybindings inside fzf (mac-friendly; avoids vim-like ctrl-h/j/k/l and F-keys):
# - Tab: expand/collapse
# - Enter: dir→cd, file→view, link→open
# - Ctrl-n / Alt-n: New directory, file, or link
# - Ctrl-r / Alt-r: Rename title
# - Ctrl-m / Alt-m: Move
# - Ctrl-d / Alt-d: Delete
# Resolve this file's directory when sourced (zsh)
__jd_script="${(%):-%N}"
__jd_dir="${__jd_script:A:h}"       # .../jd/scripts
__jd_root="${__jd_dir:h}"           # .../jd
export PATH="$__jd_root/target/release:$PATH"
unset __jd_script __jd_dir __jd_root

jd() {
  local STATE="$HOME/.cache/jd/state.json"
  local ROOTS=("/Users/justin/R50_Research")

  if [[ -z "$1" ]]; then
    # Build a literal reload command without eval; assumes paths have no spaces
    local sel
    local FZF_OPTS='--layout=reverse --no-sort --tiebreak=index'
    if [[ -n "$JD_DEBUG" ]]; then echo "[jd] launching fzf..." >&2; fi
    # Precompute tree commands for reload switching
    local TREE_BASE="jd-helper tree ${ROOTS[*]} --state \"$STATE\""
    sel=$(jd-helper tree "${ROOTS[@]}" --state "$STATE" | \
      env FZF_DEFAULT_OPTS="$FZF_OPTS" fzf --with-nth=3 --delimiter="\t" \
        --preview 'jd-helper preview --type {1} --path {4}' \
        --bind "tab:execute-silent(jd-helper toggle --state \"$STATE\" --id {2})+reload($TREE_BASE)" \
        --bind "enter:accept" \
        --bind "change:reload(
   if [ -n \"{q}\" ]; then
     ${TREE_BASE} --search '{q}'
   else
     ${TREE_BASE}
   fi
 )" \
        --bind "ctrl-a:execute-silent(jd-helper expand-all --state \"$STATE\" ${ROOTS[*]})+reload($TREE_BASE)" \
        --bind "ctrl-g:execute-silent(jd-helper reset-state --state \"$STATE\")+reload($TREE_BASE)" \
        --bind "alt-n,ctrl-n:execute(jd-helper new-interactive --parent-id {2} --display \"{3}\" ${ROOTS[@]} </dev/tty >/dev/tty 2>&1)+reload($TREE_BASE)" 
        # --bind "alt-f,ctrl-f:execute-silent(read -r -p 'New file (code + title or title): ' nm; read -r -p 'Extension (e.g., txt): ' ext; read -r -p 'Location: ' loc; nm=\${nm:-}; ext=\${ext:-txt}; test -z \"$nm\" && exit 0; code_in=\$(echo \"$nm\" | sed -E 's/^([0-9]{2}(-[0-9]{2}|\\.[0-9]{2})?)[ _-].*/\\1/'); ttl=\$(echo \"$nm\" | sed -E 's/^([0-9]{2}(-[0-9]{2}|\\.[0-9]{2})?)[ _-](.*)$/\\3/; t; s/^(.*)$/\\1/'); ttl=\$( __jd_sanitize \"$ttl\" ); pid={2}; if [[ -z \"$code_in\" || \"$code_in\" == ' ' ]]; then base=\$(__jd_extract_code_from_display \"{3}\"); if __jd_is_code_cat \"$base\"; then sug=\$(jd-helper suggest --parent \"$base\" ${ROOTS[*]}); nmf=\"\${sug}_\${ttl}.\${ext}\"; else nmf=\"\${ttl}.\${ext}\"; fi; else if __jd_is_code_item \"$code_in\"; then catc=\$(echo \"$code_in\" | cut -d. -f1); pid=\$(__jd_find_category_id_by_code \"$catc\"); nmf=\"\${code_in}_\${ttl}.\${ext}\"; elif __jd_is_code_cat \"$code_in\"; then nmf=\"\${code_in}_\${ttl}.\${ext}\"; elif __jd_is_code_range \"$code_in\"; then nmf=\"\${code_in}_\${ttl}.\${ext}\"; fi; fi; test -n \"$nmf\" && jd-helper new file --parent \"$pid\" --name \"$nmf\" --location \"$loc\" ${ROOTS[*]} )+reload($RELOAD_CMD_LIT)" \
        # --bind "alt-l,ctrl-u:execute-silent(read -r -p 'New link (code + title or title): ' nm; read -r -p 'URL: ' url; nm=\${nm:-}; test -z \"$nm\" && exit 0; test -z \"$url\" && exit 0; code_in=\$(echo \"$nm\" | sed -E 's/^([0-9]{2}(-[0-9]{2}|\\.[0-9]{2})?)[ _-].*/\\1/'); ttl=\$(echo \"$nm\" | sed -E 's/^([0-9]{2}(-[0-9]{2}|\\.[0-9]{2})?)[ _-](.*)$/\\3/; t; s/^(.*)$/\\1/'); ttl=\$( __jd_sanitize \"$ttl\" ); pid={2}; ext=webloc; if [[ -z \"$code_in\" || \"$code_in\" == ' ' ]]; then base=\$(__jd_extract_code_from_display \"{3}\"); if __jd_is_code_cat \"$base\"; then sug=\$(jd-helper suggest --parent \"$base\" ${ROOTS[*]}); nmf=\"\${sug}_\${ttl}.\${ext}\"; else nmf=\"\${ttl}.\${ext}\"; fi; else if __jd_is_code_item \"$code_in\"; then catc=\$(echo \"$code_in\" | cut -d. -f1); pid=\$(__jd_find_category_id_by_code \"$catc\"); nmf=\"\${code_in}_\${ttl}.\${ext}\"; elif __jd_is_code_cat \"$code_in\"; then nmf=\"\${code_in}_\${ttl}.\${ext}\"; elif __jd_is_code_range \"$code_in\"; then nmf=\"\${code_in}_\${ttl}.\${ext}\"; fi; fi; test -n \"$nmf\" && jd-helper new link --parent \"$pid\" --name \"$nmf\" --url \"$url\" ${ROOTS[*]} )+reload($RELOAD_CMD_LIT)"
    ) || return 1
    if [[ -n "$JD_DEBUG" ]]; then
      echo "[jd] raw sel: $sel" >&2
    fi
    local typ id disp selpath parent
    { IFS=$'\t'; read -r typ id disp selpath parent; } <<< "$sel"
    # Strip any non-letter characters from type field for robustness
    local type_clean
    type_clean=${typ//[^[:alpha:]]/}
    if [[ -n "$JD_DEBUG" ]]; then
      echo "[jd] parsed type='$typ' clean='$type_clean'" >&2
      echo "[jd] path='$selpath'" >&2
    fi
    if [[ "$type_clean" == dir ]]; then
      builtin cd -- "$selpath" || echo "[jd] cd failed: $selpath" >&2
    elif [[ "$type_clean" == link ]]; then
      open "$selpath"
    else
      if command -v vim >/dev/null 2>&1; then
        vim "$selpath"
      elif command -v bat >/dev/null 2>&1; then
        bat --style=plain --paging=always "$selpath"
      elif command -v less >/dev/null 2>&1; then
        less "$selpath"
      elif command -v more >/dev/null 2>&1; then
        more "$selpath"
      else
        cat "$selpath"
      fi
    fi
  else
    builtin cd "$(jd-helper resolve "$1" "${ROOTS[@]}")" || return 1
  fi
}


