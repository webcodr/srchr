function srchr
    set -l search_term $argv[1]

    if test -z "$search_term"
        echo "Usage: srchr <search_term>"

        return 1
    end

    set -lx SRCHR_TERM $search_term

    # locate: normalize the selected path, then find the first matching line.
    set -l locate 'file=$1; '
    set locate $locate'case $file in [-+]* ) file=./$file;; esac; '
    set locate $locate'line=$(rg -nS -m1 -- "$SRCHR_TERM" "$file" | cut -d: -f1); '

    # preview: highlight around the match, or show the whole file when none.
    set -l preview $locate
    set preview $preview'if [ -n "$line" ]; then '
    set preview $preview'start=$((line > 3 ? line - 3 : 1)); '
    set preview $preview'bat --color always --highlight-line "$line" --line-range "$start:" "$file"; '
    set preview $preview'else bat --color always "$file"; fi'

    # open: jump the editor to the match, or just open the file when none.
    set -l open $locate
    set open $open'if [ -n "$line" ]; then exec "$EDITOR" "+$line" "$file"; '
    set open $open'else exec "$EDITOR" "$file"; fi'

    begin
        fd -tf -- $search_term
        rg -lS -- $search_term
    end | sort -u | fzf \
        --preview "sh -c '$preview' sh {}" \
        --bind "enter:become(sh -c '$open' sh {})"
end
