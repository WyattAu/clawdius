# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_clawdius_global_optspecs
	string join \n n/no-tui c/cwd= f/output-format= q/quiet generate-completions= C/config= L/lang= h/help V/version
end

function __fish_clawdius_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_clawdius_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_clawdius_using_subcommand
	set -l cmd (__fish_clawdius_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c clawdius -n "__fish_clawdius_needs_command" -s c -l cwd -d 'Working directory' -r -F
complete -c clawdius -n "__fish_clawdius_needs_command" -s f -l output-format -d 'Output format (text, json, stream-json)' -r -f -a "text\t''
json\t''
stream-json\t''"
complete -c clawdius -n "__fish_clawdius_needs_command" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_needs_command" -s C -l config -d 'Path to config file (defaults to .clawdius/config.toml)' -r -F
complete -c clawdius -n "__fish_clawdius_needs_command" -s L -l lang -d 'Language for output (en, zh, ja, ko, de, fr, es, it, pt, ru)' -r
complete -c clawdius -n "__fish_clawdius_needs_command" -s n -l no-tui -d 'Run without TUI (headless mode)'
complete -c clawdius -n "__fish_clawdius_needs_command" -s q -l quiet -d 'Quiet mode (no progress indicators)'
complete -c clawdius -n "__fish_clawdius_needs_command" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_needs_command" -s V -l version -d 'Print version'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "chat" -d 'Send a chat message to the LLM'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "auto" -d 'Autonomous CI/CD mode - run without interaction'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "init" -d 'Initialize a new Clawdius project in the current directory'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "setup" -d 'Interactive setup wizard for first-time users'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "sessions" -d 'List and manage sessions'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "refactor" -d 'Plan and execute a cross-language refactor'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "action" -d 'Apply a code action'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "test" -d 'Generate tests for code'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "doc" -d 'Generate documentation for code'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "verify" -d 'Run Lean4 proof verification'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "metrics" -d 'Show performance metrics'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "telemetry" -d 'Configure telemetry settings'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "checkpoint" -d 'Manage file checkpoints'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "timeline" -d 'Manage file timeline and version history'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "modes" -d 'Manage agent modes'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "lang" -d 'Manage language settings'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "edit" -d 'Edit a long prompt in external editor'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "webhook" -d 'Manage webhooks for event notifications'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "generate" -d 'Generate code using agentic AI'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "lsp" -d 'Language Server Protocol operations'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "memory" -d 'Manage project memory (CLAWDIUS.md)'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "models" -d 'Manage local LLM models (Ollama)'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "complete" -d 'Get inline code completions from LLM'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "analyze" -d 'Analyze codebase for architecture drift and technical debt'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "watch" -d 'Watch files for changes and trigger auto-analysis'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "git" -d 'Git workflow operations'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "sprint" -d 'Run an agentic sprint (think -> plan -> build -> review -> test -> ship -> reflect)'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "ship" -d 'Run pre-ship checks or generate a commit message'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "skill" -d 'List and execute markdown skills'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "server" -d 'Start the Clawdius HTTP server'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "config" -d 'View and manage configuration'
complete -c clawdius -n "__fish_clawdius_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -s m -l model -d 'Model to use (defaults to provider\'s default model)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -s P -l provider -d 'Provider to use (anthropic, openai, deepseek, ollama, zai, openrouter)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -s s -l session -d 'Continue from session ID' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -s M -l mode -d 'Agent mode (code, architect, ask, debug, review, refactor, test, auto, or custom mode name)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -s e -l editor -d 'Open external editor to compose message'
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -l exit -d 'Non-interactive mode - exit after response (auto-enabled when prompt provided)'
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -l quiet -d 'Quiet mode (suppress all output except response)'
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -l auto-approve -d 'Autonomous mode - auto-approve all tool executions'
complete -c clawdius -n "__fish_clawdius_using_subcommand chat" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -s m -l model -d 'Model to use (defaults to provider\'s default model)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -s P -l provider -d 'Provider to use (anthropic, openai, deepseek, ollama, zai, openrouter)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -l max-iterations -d 'Maximum iterations before stopping (default: 50)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -l output-format -d 'Output format for CI logging (text, json, github-actions)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -l run-tests -d 'Run tests after changes'
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -l auto-commit -d 'Commit changes automatically'
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -l fail-on-test-failure -d 'Fail if tests fail after changes'
complete -c clawdius -n "__fish_clawdius_using_subcommand auto" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand init" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand init" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand setup" -s P -l provider -d 'Pre-select provider (anthropic, openai, ollama, zai)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand setup" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand setup" -s q -l quick -d 'Skip welcome screen'
complete -c clawdius -n "__fish_clawdius_using_subcommand setup" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand sessions" -s d -l delete -d 'Delete a session' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sessions" -s s -l search -d 'Search sessions' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sessions" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sessions" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand refactor" -s f -l from -d 'Source language (e.g., typescript, python)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand refactor" -s t -l to -d 'Target language (e.g., rust, go)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand refactor" -s p -l path -d 'Path to file or directory' -r -F
complete -c clawdius -n "__fish_clawdius_using_subcommand refactor" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand refactor" -l dry-run -d 'Preview changes without applying'
complete -c clawdius -n "__fish_clawdius_using_subcommand refactor" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand action" -s l -l line -d 'Line number' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand action" -s c -l column -d 'Column number' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand action" -s s -l end-line -d 'End line for selection' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand action" -l end-column -d 'End column for selection' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand action" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand action" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand test" -s f -l function -d 'Function name to generate tests for (generates for all if not specified)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand test" -s o -l output -d 'Output file path (defaults to <file>_test.<ext>)' -r -F
complete -c clawdius -n "__fish_clawdius_using_subcommand test" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand test" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand doc" -s e -l element -d 'Element to document (function, struct, module)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand doc" -s f -l format -d 'Documentation format (auto, rustdoc, jsdoc, pydoc, markdown)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand doc" -s o -l output -d 'Output file path (defaults to stdout)' -r -F
complete -c clawdius -n "__fish_clawdius_using_subcommand doc" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand doc" -l inline -d 'Include inline comments'
complete -c clawdius -n "__fish_clawdius_using_subcommand doc" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand verify" -s p -l proof -d 'Path to .lean proof file or directory' -r -F
complete -c clawdius -n "__fish_clawdius_using_subcommand verify" -l lean-path -d 'Path to lean binary' -r -F
complete -c clawdius -n "__fish_clawdius_using_subcommand verify" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand verify" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand metrics" -s f -l format -d 'Output format (text, json, html)' -r -f -a "text\t''
json\t''
html\t''"
complete -c clawdius -n "__fish_clawdius_using_subcommand metrics" -s o -l output -d 'Output file path (prints to stdout if not specified)' -r -F
complete -c clawdius -n "__fish_clawdius_using_subcommand metrics" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand metrics" -s r -l reset -d 'Reset metrics after displaying'
complete -c clawdius -n "__fish_clawdius_using_subcommand metrics" -s w -l watch -d 'Watch mode - continuously display metrics'
complete -c clawdius -n "__fish_clawdius_using_subcommand metrics" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand telemetry" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand telemetry" -s e -l enable -d 'Enable telemetry'
complete -c clawdius -n "__fish_clawdius_using_subcommand telemetry" -s d -l disable -d 'Disable telemetry'
complete -c clawdius -n "__fish_clawdius_using_subcommand telemetry" -l enable-metrics -d 'Enable metrics collection'
complete -c clawdius -n "__fish_clawdius_using_subcommand telemetry" -l enable-crash-reporting -d 'Enable crash reporting'
complete -c clawdius -n "__fish_clawdius_using_subcommand telemetry" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "create" -d 'Create a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "list" -d 'List all checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "show" -d 'Show checkpoint details'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "restore" -d 'Restore to a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "compare" -d 'Compare two checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "delete" -d 'Delete a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "cleanup" -d 'Clean up old checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "timeline" -d 'Show checkpoint timeline'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and not __fish_seen_subcommand_from create list show restore compare delete cleanup timeline help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from create" -s s -l session -d 'Session ID (defaults to current session)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from create" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from list" -s s -l session -d 'Session ID (defaults to current session)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from list" -s v -l verbose -d 'Show file details'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from show" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from restore" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from restore" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from compare" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from compare" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from delete" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from delete" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from cleanup" -s s -l session -d 'Session ID (defaults to current session)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from cleanup" -s k -l keep -d 'Number of checkpoints to keep' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from cleanup" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from cleanup" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from timeline" -s s -l session -d 'Session ID (defaults to current session)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from timeline" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from timeline" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "list" -d 'List all checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "show" -d 'Show checkpoint details'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "restore" -d 'Restore to a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "compare" -d 'Compare two checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "delete" -d 'Delete a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "cleanup" -d 'Clean up old checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "timeline" -d 'Show checkpoint timeline'
complete -c clawdius -n "__fish_clawdius_using_subcommand checkpoint; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "create" -d 'Create a timeline checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "list" -d 'List all timeline checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "watch" -d 'Watch for file changes and auto-create checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "rollback" -d 'Rollback to a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "diff" -d 'Show diff between two checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "history" -d 'Show file history'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "delete" -d 'Delete a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "cleanup" -d 'Clean up old checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and not __fish_seen_subcommand_from create list watch rollback diff history delete cleanup help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from create" -s d -l description -d 'Description for the checkpoint' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from create" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from watch" -s d -l debounce-secs -d 'Debounce interval in seconds' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from watch" -s i -l ignore -d 'Additional patterns to ignore (can be repeated)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from watch" -s m -l max-per-hour -d 'Maximum checkpoints per hour' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from watch" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from watch" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from rollback" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from rollback" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from diff" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from diff" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from history" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from history" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from delete" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from delete" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from cleanup" -s k -l keep -d 'Number of checkpoints to keep' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from cleanup" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from cleanup" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a timeline checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "list" -d 'List all timeline checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "watch" -d 'Watch for file changes and auto-create checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "rollback" -d 'Rollback to a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "diff" -d 'Show diff between two checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "history" -d 'Show file history'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "delete" -d 'Delete a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "cleanup" -d 'Clean up old checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand timeline; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and not __fish_seen_subcommand_from list create show help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and not __fish_seen_subcommand_from list create show help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and not __fish_seen_subcommand_from list create show help" -f -a "list" -d 'List all available modes'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and not __fish_seen_subcommand_from list create show help" -f -a "create" -d 'Create a new custom mode'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and not __fish_seen_subcommand_from list create show help" -f -a "show" -d 'Show details of a mode'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and not __fish_seen_subcommand_from list create show help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from create" -s o -l output -d 'Path to save mode configuration' -r -F
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from create" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from show" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from help" -f -a "list" -d 'List all available modes'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a new custom mode'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from help" -f -a "show" -d 'Show details of a mode'
complete -c clawdius -n "__fish_clawdius_using_subcommand modes; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and not __fish_seen_subcommand_from list set show help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and not __fish_seen_subcommand_from list set show help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and not __fish_seen_subcommand_from list set show help" -f -a "list" -d 'List supported languages'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and not __fish_seen_subcommand_from list set show help" -f -a "set" -d 'Set display language'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and not __fish_seen_subcommand_from list set show help" -f -a "show" -d 'Show current language'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and not __fish_seen_subcommand_from list set show help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from set" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from set" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from show" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from help" -f -a "list" -d 'List supported languages'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from help" -f -a "set" -d 'Set display language'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from help" -f -a "show" -d 'Show current language'
complete -c clawdius -n "__fish_clawdius_using_subcommand lang; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand edit" -s i -l initial -d 'Optional initial content' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand edit" -s e -l editor -d 'Editor to use (defaults to $EDITOR)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand edit" -s x -l extension -d 'File extension for syntax highlighting (default: md)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand edit" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand edit" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "list" -d 'List all webhooks'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "create" -d 'Create a new webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "show" -d 'Show webhook details'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "update" -d 'Update a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "delete" -d 'Delete a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "test" -d 'Test a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "deliveries" -d 'Show delivery history'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "stats" -d 'Show webhook statistics'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and not __fish_seen_subcommand_from list create show update delete test deliveries stats help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from create" -s E -l events -d 'Events to subscribe to (comma-separated)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from create" -s s -l secret -d 'Secret for signature verification' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from create" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from show" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from update" -s u -l url -d 'New target URL' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from update" -s E -l events -d 'New events (comma-separated)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from update" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from update" -s e -l enable -d 'Enable webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from update" -s D -l disable -d 'Disable webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from update" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from delete" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from delete" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from test" -s e -l event -d 'Event type to test' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from test" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from test" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from deliveries" -s n -l limit -d 'Number of deliveries to show' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from deliveries" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from deliveries" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from stats" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from stats" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "list" -d 'List all webhooks'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a new webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "show" -d 'Show webhook details'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "update" -d 'Update a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "delete" -d 'Delete a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "test" -d 'Test a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "deliveries" -d 'Show delivery history'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "stats" -d 'Show webhook statistics'
complete -c clawdius -n "__fish_clawdius_using_subcommand webhook; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s f -l files -d 'Target files to generate/modify (comma-separated)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s M -l mode -d 'Generation mode: single-pass, iterative, agent' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s T -l trust -d 'Trust level for apply: low, medium, high' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s t -l test-strategy -d 'Test execution strategy: sandboxed, direct, skip' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s i -l max-iterations -d 'Max iterations for iterative/agent mode' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s P -l provider -d 'LLM provider to use' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s m -l model -d 'Model to use' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s R -l timeout-secs -d 'Timeout in seconds for LLM operations' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -l dry-run -d 'Dry run - preview changes without applying'
complete -c clawdius -n "__fish_clawdius_using_subcommand generate" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "start" -d 'Start an LSP server for a language'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "complete" -d 'Get completions at a position'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "hover" -d 'Get hover information at a position'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "definition" -d 'Go to definition'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "references" -d 'Find references'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "symbols" -d 'Get document symbols'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "diagnostics" -d 'Get diagnostics for a file'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "code-actions" -d 'Get code actions for a range'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and not __fish_seen_subcommand_from start complete hover definition references symbols diagnostics code-actions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from start" -s r -l root -d 'Root URI for the workspace' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from start" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from start" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from complete" -s l -l line -d 'Line number (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from complete" -s c -l column -d 'Column number (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from complete" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from complete" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from hover" -s l -l line -d 'Line number (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from hover" -s c -l column -d 'Column number (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from hover" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from hover" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from definition" -s l -l line -d 'Line number (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from definition" -s c -l column -d 'Column number (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from definition" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from definition" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from references" -s l -l line -d 'Line number (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from references" -s c -l column -d 'Column number (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from references" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from references" -l include-declaration -d 'Include declaration'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from references" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from symbols" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from symbols" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from diagnostics" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from diagnostics" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from code-actions" -s l -l start-line -d 'Start line (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from code-actions" -s c -l start-column -d 'Start column (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from code-actions" -s L -l end-line -d 'End line (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from code-actions" -s C -l end-column -d 'End column (0-indexed)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from code-actions" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from code-actions" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "start" -d 'Start an LSP server for a language'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "complete" -d 'Get completions at a position'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "hover" -d 'Get hover information at a position'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "definition" -d 'Go to definition'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "references" -d 'Find references'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "symbols" -d 'Get document symbols'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "diagnostics" -d 'Get diagnostics for a file'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "code-actions" -d 'Get code actions for a range'
complete -c clawdius -n "__fish_clawdius_using_subcommand lsp; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -f -a "show" -d 'Show project memory (CLAWDIUS.md + learned entries)'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -f -a "learn" -d 'Learn a new memory entry'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -f -a "instructions" -d 'Set project instructions'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -f -a "list" -d 'List learned entries by category'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -f -a "clear" -d 'Clear learned entries'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -f -a "init" -d 'Create or update CLAWDIUS.md file'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and not __fish_seen_subcommand_from show learn instructions list clear init help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from show" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from show" -s i -l instructions -d 'Show as LLM-ready instructions'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from learn" -s d -l description -d 'Optional description' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from learn" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from learn" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from instructions" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from instructions" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from clear" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from clear" -s y -l yes -d 'Confirm clearing all entries'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from clear" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from init" -s n -l name -d 'Project name' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from init" -s L -l language -d 'Primary language' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from init" -s f -l framework -d 'Framework' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from init" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from init" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from help" -f -a "show" -d 'Show project memory (CLAWDIUS.md + learned entries)'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from help" -f -a "learn" -d 'Learn a new memory entry'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from help" -f -a "instructions" -d 'Set project instructions'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from help" -f -a "list" -d 'List learned entries by category'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from help" -f -a "clear" -d 'Clear learned entries'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from help" -f -a "init" -d 'Create or update CLAWDIUS.md file'
complete -c clawdius -n "__fish_clawdius_using_subcommand memory; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -s H -l host -d 'Ollama host' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -s p -l port -d 'Ollama port' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -f -a "list" -d 'List available local models'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -f -a "pull" -d 'Pull a model from registry'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -f -a "health" -d 'Check Ollama server health'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -f -a "current" -d 'Show current model'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and not __fish_seen_subcommand_from list pull health current help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from pull" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from pull" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from health" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from health" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from current" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from current" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from help" -f -a "list" -d 'List available local models'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from help" -f -a "pull" -d 'Pull a model from registry'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from help" -f -a "health" -d 'Check Ollama server health'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from help" -f -a "current" -d 'Show current model'
complete -c clawdius -n "__fish_clawdius_using_subcommand models; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand complete" -s l -l language -d 'Programming language' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand complete" -s P -l provider -d 'LLM provider to use' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand complete" -s m -l model -d 'Model name' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand complete" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand complete" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand analyze" -s f -l format -d 'Output format (text, json)' -r -f -a "text\t''
json\t''
stream-json\t''"
complete -c clawdius -n "__fish_clawdius_using_subcommand analyze" -s o -l output -d 'Output file path (prints to stdout if not specified)' -r -F
complete -c clawdius -n "__fish_clawdius_using_subcommand analyze" -l severity -d 'Minimum severity level to report (low, medium, high, critical)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand analyze" -l exclude -d 'Exclude patterns (comma-separated)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand analyze" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand analyze" -l drift -d 'Analyze for architecture drift only'
complete -c clawdius -n "__fish_clawdius_using_subcommand analyze" -l debt -d 'Analyze for technical debt only'
complete -c clawdius -n "__fish_clawdius_using_subcommand analyze" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand watch" -l ignore -d 'Patterns to ignore (comma-separated)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand watch" -l debounce-ms -d 'Debounce interval in milliseconds' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand watch" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand watch" -l auto-analyze -d 'Enable auto-analysis on changes'
complete -c clawdius -n "__fish_clawdius_using_subcommand watch" -s v -l verbose -d 'Enable verbose output'
complete -c clawdius -n "__fish_clawdius_using_subcommand watch" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and not __fish_seen_subcommand_from commit diff status help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and not __fish_seen_subcommand_from commit diff status help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and not __fish_seen_subcommand_from commit diff status help" -f -a "commit" -d 'Stage files and create a commit with an LLM-generated message'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and not __fish_seen_subcommand_from commit diff status help" -f -a "diff" -d 'Show a diff of staged or modified files'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and not __fish_seen_subcommand_from commit diff status help" -f -a "status" -d 'Show git status summary'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and not __fish_seen_subcommand_from commit diff status help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from commit" -s m -l message -d 'Commit message hint (optional)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from commit" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from commit" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from diff" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from diff" -s s -l staged -d 'Show staged diff instead of working diff'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from diff" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from status" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from help" -f -a "commit" -d 'Stage files and create a commit with an LLM-generated message'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from help" -f -a "diff" -d 'Show a diff of staged or modified files'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from help" -f -a "status" -d 'Show git status summary'
complete -c clawdius -n "__fish_clawdius_using_subcommand git; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -s n -l max-iterations -d 'Maximum build-test iterations' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -s P -l provider -d 'LLM provider' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -s m -l model -d 'Specific model to use' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -l browser-qa-url -d 'URL for browser-based QA in the Test phase' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -l lsp -d 'LSP server command for code intelligence (e.g., --lsp "rust-analyzer")' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -l real-execution -d 'Enable real command execution (build, test)'
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -l auto-approve -d 'Auto-approve all phases without confirmation'
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -l resume -d 'Resume from the most recent saved sprint state'
complete -c clawdius -n "__fish_clawdius_using_subcommand sprint" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and not __fish_seen_subcommand_from checks commit-message help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and not __fish_seen_subcommand_from checks commit-message help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and not __fish_seen_subcommand_from checks commit-message help" -f -a "checks" -d 'Run pre-ship quality checks'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and not __fish_seen_subcommand_from checks commit-message help" -f -a "commit-message" -d 'Generate a conventional commit message'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and not __fish_seen_subcommand_from checks commit-message help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from checks" -s b -l branch -d 'Branch name (default: current branch)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from checks" -s f -l files -d 'Changed files to check' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from checks" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from checks" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from commit-message" -s f -l files -d 'Changed files' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from commit-message" -s d -l description -d 'Description of the changes' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from commit-message" -s s -l scope -d 'Commit scope (e.g. "core", "api")' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from commit-message" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from commit-message" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from help" -f -a "checks" -d 'Run pre-ship quality checks'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from help" -f -a "commit-message" -d 'Generate a conventional commit message'
complete -c clawdius -n "__fish_clawdius_using_subcommand ship; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and not __fish_seen_subcommand_from list run help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and not __fish_seen_subcommand_from list run help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and not __fish_seen_subcommand_from list run help" -f -a "list" -d 'List available skills'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and not __fish_seen_subcommand_from list run help" -f -a "run" -d 'Execute a skill by name'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and not __fish_seen_subcommand_from list run help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and __fish_seen_subcommand_from run" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and __fish_seen_subcommand_from run" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and __fish_seen_subcommand_from help" -f -a "list" -d 'List available skills'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and __fish_seen_subcommand_from help" -f -a "run" -d 'Execute a skill by name'
complete -c clawdius -n "__fish_clawdius_using_subcommand skill; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand server" -l host -r
complete -c clawdius -n "__fish_clawdius_using_subcommand server" -s p -l port -r
complete -c clawdius -n "__fish_clawdius_using_subcommand server" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand server" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and not __fish_seen_subcommand_from show get set path list help" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and not __fish_seen_subcommand_from show get set path list help" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and not __fish_seen_subcommand_from show get set path list help" -f -a "show" -d 'Show current configuration (masks API keys)'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and not __fish_seen_subcommand_from show get set path list help" -f -a "get" -d 'Get a specific config value'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and not __fish_seen_subcommand_from show get set path list help" -f -a "set" -d 'Set a specific config value'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and not __fish_seen_subcommand_from show get set path list help" -f -a "path" -d 'Show the path to the config file'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and not __fish_seen_subcommand_from show get set path list help" -f -a "list" -d 'List available config keys'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and not __fish_seen_subcommand_from show get set path list help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from show" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from get" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from get" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from set" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from set" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from path" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from path" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from list" -l generate-completions -d 'Generate shell completions to stdout (bash|zsh|fish|powershell)' -r
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "show" -d 'Show current configuration (masks API keys)'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "get" -d 'Get a specific config value'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "set" -d 'Set a specific config value'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "path" -d 'Show the path to the config file'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "list" -d 'List available config keys'
complete -c clawdius -n "__fish_clawdius_using_subcommand config; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "chat" -d 'Send a chat message to the LLM'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "auto" -d 'Autonomous CI/CD mode - run without interaction'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "init" -d 'Initialize a new Clawdius project in the current directory'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "setup" -d 'Interactive setup wizard for first-time users'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "sessions" -d 'List and manage sessions'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "refactor" -d 'Plan and execute a cross-language refactor'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "action" -d 'Apply a code action'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "test" -d 'Generate tests for code'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "doc" -d 'Generate documentation for code'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "verify" -d 'Run Lean4 proof verification'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "metrics" -d 'Show performance metrics'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "telemetry" -d 'Configure telemetry settings'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "checkpoint" -d 'Manage file checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "timeline" -d 'Manage file timeline and version history'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "modes" -d 'Manage agent modes'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "lang" -d 'Manage language settings'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "edit" -d 'Edit a long prompt in external editor'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "webhook" -d 'Manage webhooks for event notifications'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "generate" -d 'Generate code using agentic AI'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "lsp" -d 'Language Server Protocol operations'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "memory" -d 'Manage project memory (CLAWDIUS.md)'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "models" -d 'Manage local LLM models (Ollama)'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "complete" -d 'Get inline code completions from LLM'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "analyze" -d 'Analyze codebase for architecture drift and technical debt'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "watch" -d 'Watch files for changes and trigger auto-analysis'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "git" -d 'Git workflow operations'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "sprint" -d 'Run an agentic sprint (think -> plan -> build -> review -> test -> ship -> reflect)'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "ship" -d 'Run pre-ship checks or generate a commit message'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "skill" -d 'List and execute markdown skills'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "server" -d 'Start the Clawdius HTTP server'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "config" -d 'View and manage configuration'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and not __fish_seen_subcommand_from chat auto init setup sessions refactor action test doc verify metrics telemetry checkpoint timeline modes lang edit webhook generate lsp memory models complete analyze watch git sprint ship skill server config help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "create" -d 'Create a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "list" -d 'List all checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "show" -d 'Show checkpoint details'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "restore" -d 'Restore to a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "compare" -d 'Compare two checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "delete" -d 'Delete a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "cleanup" -d 'Clean up old checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from checkpoint" -f -a "timeline" -d 'Show checkpoint timeline'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from timeline" -f -a "create" -d 'Create a timeline checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from timeline" -f -a "list" -d 'List all timeline checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from timeline" -f -a "watch" -d 'Watch for file changes and auto-create checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from timeline" -f -a "rollback" -d 'Rollback to a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from timeline" -f -a "diff" -d 'Show diff between two checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from timeline" -f -a "history" -d 'Show file history'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from timeline" -f -a "delete" -d 'Delete a checkpoint'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from timeline" -f -a "cleanup" -d 'Clean up old checkpoints'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from modes" -f -a "list" -d 'List all available modes'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from modes" -f -a "create" -d 'Create a new custom mode'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from modes" -f -a "show" -d 'Show details of a mode'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lang" -f -a "list" -d 'List supported languages'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lang" -f -a "set" -d 'Set display language'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lang" -f -a "show" -d 'Show current language'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from webhook" -f -a "list" -d 'List all webhooks'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from webhook" -f -a "create" -d 'Create a new webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from webhook" -f -a "show" -d 'Show webhook details'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from webhook" -f -a "update" -d 'Update a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from webhook" -f -a "delete" -d 'Delete a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from webhook" -f -a "test" -d 'Test a webhook'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from webhook" -f -a "deliveries" -d 'Show delivery history'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from webhook" -f -a "stats" -d 'Show webhook statistics'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "start" -d 'Start an LSP server for a language'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "complete" -d 'Get completions at a position'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "hover" -d 'Get hover information at a position'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "definition" -d 'Go to definition'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "references" -d 'Find references'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "symbols" -d 'Get document symbols'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "diagnostics" -d 'Get diagnostics for a file'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from lsp" -f -a "code-actions" -d 'Get code actions for a range'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from memory" -f -a "show" -d 'Show project memory (CLAWDIUS.md + learned entries)'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from memory" -f -a "learn" -d 'Learn a new memory entry'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from memory" -f -a "instructions" -d 'Set project instructions'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from memory" -f -a "list" -d 'List learned entries by category'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from memory" -f -a "clear" -d 'Clear learned entries'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from memory" -f -a "init" -d 'Create or update CLAWDIUS.md file'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from models" -f -a "list" -d 'List available local models'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from models" -f -a "pull" -d 'Pull a model from registry'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from models" -f -a "health" -d 'Check Ollama server health'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from models" -f -a "current" -d 'Show current model'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from git" -f -a "commit" -d 'Stage files and create a commit with an LLM-generated message'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from git" -f -a "diff" -d 'Show a diff of staged or modified files'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from git" -f -a "status" -d 'Show git status summary'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from ship" -f -a "checks" -d 'Run pre-ship quality checks'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from ship" -f -a "commit-message" -d 'Generate a conventional commit message'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from skill" -f -a "list" -d 'List available skills'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from skill" -f -a "run" -d 'Execute a skill by name'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from config" -f -a "show" -d 'Show current configuration (masks API keys)'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from config" -f -a "get" -d 'Get a specific config value'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from config" -f -a "set" -d 'Set a specific config value'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from config" -f -a "path" -d 'Show the path to the config file'
complete -c clawdius -n "__fish_clawdius_using_subcommand help; and __fish_seen_subcommand_from config" -f -a "list" -d 'List available config keys'
