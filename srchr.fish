function srchr
    set -l search_term $argv[1]

    if test -z "$search_term"
        echo "Usage: srchr <search_term>"

        return 1
    end

    set -lx SRCHR_TERM $search_term

    begin
        fd -tf -- $search_term
        rg -lS -- $search_term
    end | sort -u | fzf \
        --preview 'sh -c \'file=$1; case $file in [-+]* ) file=./$file;; esac; line=$(rg -nS -m1 -- "$SRCHR_TERM" "$file" | cut -d: -f1); if [ -n "$line" ]; then start=$((line > 3 ? line - 3 : 1)); bat --color always --highlight-line "$line" --line-range "$start:" "$file"; else bat --color always "$file"; fi\' sh {}' \
        --bind 'enter:become(sh -c \'file=$1; case $file in [-+]* ) file=./$file;; esac; line=$(rg -nS -m1 -- "$SRCHR_TERM" "$file" | cut -d: -f1); if [ -n "$line" ]; then exec "$EDITOR" "+$line" "$file"; else exec "$EDITOR" "$file"; fi\' sh {})'
end
