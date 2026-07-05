# Unified fd + rg search for bash and zsh. Source this file, then run:
#   srchr <search_term>
srchr() {
    local search_term=$1

    if [ -z "$search_term" ]; then
        echo "Usage: srchr <search_term>"

        return 1
    fi

    # locate: normalize the selected path, then find the first matching line.
    local locate='file=$1; '
    locate=$locate'case $file in [-+]* ) file=./$file;; esac; '
    locate=$locate'line=$(rg -nS -m1 -- "$SRCHR_TERM" "$file" | cut -d: -f1); '

    # preview: highlight around the match, or show the whole file when none.
    local preview=$locate
    preview=$preview'if [ -n "$line" ]; then '
    preview=$preview'start=$((line > 3 ? line - 3 : 1)); '
    preview=$preview'bat --color always --highlight-line "$line" --line-range "$start:" "$file"; '
    preview=$preview'else bat --color always "$file"; fi'

    # open: jump the editor to the match, or just open the file when none.
    local open=$locate
    open=$open'if [ -n "$line" ]; then exec "$EDITOR" "+$line" "$file"; '
    open=$open'else exec "$EDITOR" "$file"; fi'

    {
        fd -tf -- "$search_term"
        rg -lS -- "$search_term"
    } | sort -u | SRCHR_TERM=$search_term fzf \
        --preview "sh -c '$preview' sh {}" \
        --bind "enter:become(sh -c '$open' sh {})"
}
