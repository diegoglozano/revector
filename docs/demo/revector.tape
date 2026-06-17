# revector demo tape — regenerate the README GIF with:  vhs revector.tape
# Prerequisites:
#   * `revector` on your PATH        (cargo build --release, then add target/release)
#   * a Qdrant reachable at $REVECTOR_URL (defaults to http://localhost:6334)
#   * `migration.yaml` sitting next to this tape
Output revector.gif

Require revector

Set Shell bash
Set FontSize 16
Set Width 1180
Set Height 760
Set Padding 20
Set Margin 10
Set BorderRadius 8
Set Theme "Dracula"
Set TypingSpeed 30ms
Set Framerate 24

# --- hidden setup: remember source dir, use a scratch workspace ---
Hide
Type "SRC=$PWD" Enter
Type "export REVECTOR_URL=${REVECTOR_URL:-http://localhost:6334}" Enter
Type "export PS1='\033[1;32m$\033[0m '" Enter
Type "WORK=$(mktemp -d); cd $WORK" Enter
Type "clear" Enter
Show

Sleep 800ms

# 1. scaffold a project
Type "revector init" Sleep 400ms Enter
Sleep 1.6s

# 2. create a migration
Type 'revector new "create products collection"' Sleep 400ms Enter
Sleep 1.8s

# 3. drop in the (pre-written) migration body, then review it
Hide
Type 'cp "$SRC/migration.yaml" migrations/*_create_products_collection.yaml' Enter
Type "clear" Enter
Show
Type "# ...edit the migration in your editor, then review it:" Sleep 300ms Enter
Sleep 700ms
Type "cat migrations/*_create_products_collection.yaml" Sleep 400ms Enter
Sleep 3.5s

# 4. apply it
Type "revector up --yes" Sleep 400ms Enter
Sleep 2.6s

# 5. inspect state
Type "revector status" Sleep 400ms Enter
Sleep 2.8s

# 6. roll it back — note the auto-inverted ops
Type "revector down --yes" Sleep 400ms Enter
Sleep 3.2s

Sleep 1.5s
