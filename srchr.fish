function srchr
    set -l search_term $argv[1]

    if test -z "$search_term"
        echo "Usage: srchr <search_term>"

        return 1
    end

    set -lx SRCHR_TERM $search_term

    begin
        fd -tf $search_term
        rg -lS $search_term
    end | sort -u | fzf \
        --preview 'sh -c \'line=$(rg -nS -m1 -- "$SRCHR_TERM" "$1" | cut -d: -f1); if [ -n "$line" ]; then start=$((line > 3 ? line - 3 : 1)); bat --color always --highlight-line "$line" --line-range "$start:" "$1"; else bat --color always "$1"; fi\' sh {}' \
        --bind 'enter:become(sh -c \'line=$(rg -nS -m1 -- "$SRCHR_TERM" "$1" | cut -d: -f1); if [ -n "$line" ]; then exec "$EDITOR" "+$line" "$1"; else exec "$EDITOR" "$1"; fi\' sh {})'
end
