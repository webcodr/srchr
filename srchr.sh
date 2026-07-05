# Unified fd + rg search for bash and zsh. Source this file, then run:
#   srchr <search_term>
srchr() {
    local search_term=$1

    if [ -z "$search_term" ]; then
        echo "Usage: srchr <search_term>"

        return 1
    fi

    local locate='line=$(rg -nS -m1 -- "$SRCHR_TERM" "$1" | cut -d: -f1); '
    local preview=$locate'if [ -n "$line" ]; then start=$((line > 3 ? line - 3 : 1)); bat --color always --highlight-line "$line" --line-range "$start:" "$1"; else bat --color always "$1"; fi'
    local open=$locate'if [ -n "$line" ]; then exec "$EDITOR" "+$line" "$1"; else exec "$EDITOR" "$1"; fi'

    {
        fd -tf "$search_term"
        rg -lS "$search_term"
    } | sort -u | SRCHR_TERM=$search_term fzf \
        --preview "sh -c '$preview' sh {}" \
        --bind "enter:become(sh -c '$open' sh {})"
}
