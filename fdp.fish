function fdp
    set -l search_term $argv[1]

    if test -z "$search_term"
        echo "Usage: fdp <search_term>"

        return 1
    end

    fd -tf $search_term | fzf --preview 'bat --color always {}' --bind 'enter:become("$EDITOR" {+})'
end
