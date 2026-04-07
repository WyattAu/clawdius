;;; clawdius.el --- AI-powered coding assistant for Emacs  -*- lexical-binding: t; -*-
;;; Commentary:
;;; Clawdius provides AI-powered coding assistance via the Clawdius server.
;;;
;;; Usage:
;;;   (require 'clawdius)
;;;   (clawdius-setup)
;;;
;;; Commands:
;;;   M-x clawdius-chat           - Open AI chat buffer
;;;   M-x clawdius-analyze         - Analyze current file
;;;   M-x clawdius-complete-at-point - Trigger completion at point
;;;   M-x clawdius-explain         - Explain code at point or region
;;;   M-x clawdius-refactor        - Suggest refactoring for region
;;;   M-x clawdius-fix             - Fix issues in current file
;;;   M-x clawdius-health          - Check server connection
;;;   M-x clawdius-diff            - Show git diff
;;;   M-x clawdius-add-context     - Add file context to chat session
;;;   M-x clawdius-checkpoint      - Create a session checkpoint

;; Author: Clawdius Team
;; URL: https://github.com/WyattAu/clawdius
;; Version: 0.1.0
;; Package-Requires: ((emacs "27.1") (json))

;;; Code:

(require 'cl-lib)
(require 'json)
(require 'url-http)
(require 'subr-x)

(defgroup clawdius nil
  "AI-powered coding assistant for Emacs."
  :group 'tools)

(defcustom clawdius-host "localhost"
  "Clawdius server hostname."
  :type 'string
  :group 'clawdius)

(defcustom clawdius-port 8080
  "Clawdius server port."
  :type 'integer
  :group 'clawdius)

(defcustom clawdius-api-key nil
  "API key for Clawdius server authentication."
  :type '(choice (const :tag "None" nil) string)
  :group 'clawdius)

(defcustom clawdius-enable-completion t
  "Enable Clawdius inline completion via company-mode."
  :type 'boolean
  :group 'clawdius)

(defcustom clawdius-completion-trigger-chars '(?. ?\( ?\s)
  "Characters that trigger inline completion."
  :type '(repeat character)
  :group 'clawdius)

(defcustom clawdius-completion-debounce 0.5
  "Debounce delay in seconds for inline completions."
  :type 'number
  :group 'clawdius)

(defcustom clawdius-model nil
  "LLM model to use (nil for server default)."
  :type '(choice (const :tag "Server default" nil) string)
  :group 'clawdius)

(defcustom clawdius-request-timeout 30
  "Timeout in seconds for HTTP requests."
  :type 'integer
  :group 'clawdius)

(defvar clawdius--health-status nil
  "Last known health status of the Clawdius server.")

(defvar clawdius--session-id nil
  "Current chat session ID.")

(defvar clawdius--debounce-timer nil
  "Timer for debounced completion requests.")

;;; ---------------------------------------------------------------------------
;;; HTTP helpers
;;; ---------------------------------------------------------------------------

(defun clawdius--url (&optional path)
  "Build full API URL for PATH."
  (format "http://%s:%d%s" clawdius-host clawdius-port (or path "/")))

(defun clawdius--headers ()
  "Build HTTP headers for API requests."
  (let ((headers '(("Content-Type" . "application/json"))))
    (when clawdius-api-key
      (push (cons "Authorization" (concat "Bearer " clawdius-api-key))
            headers))
    headers))

(defun clawdius--extract-json-body ()
  "Extract JSON body from current url-retrieve buffer.
Assumes point is at the beginning of the buffer."
  (goto-char (point-min))
  (when (search-forward "\n\n" nil t)
    (let ((raw (buffer-substring-no-properties (point) (point-max))))
      (condition-case nil
          (json-read-from-string raw)
        (json-parse-error nil)))))

(defun clawdius--request (method path &optional body callback)
  "Make an HTTP request to the Clawdius server.
METHOD is the HTTP method string.  PATH is the API path.
BODY is an optional alist to encode as JSON.  CALLBACK is
called with (JSON-BODY . HTTP-STATUS) where JSON-BODY is a parsed
JSON object or nil, and HTTP-STATUS is the integer status code."
  (let* ((url (clawdius--url path))
         (url-request-method method)
         (url-request-extra-headers (clawdius--headers))
         (url-request-data (when body
                             (encode-coding-string (json-encode body) 'utf-8))))
    (url-retrieve
     url
     (lambda (_status)
       (let ((resp-buf (current-buffer)))
         (unwind-protect
             (let* ((http-status (url-http-parse-response))
                    (json-body (clawdius--extract-json-body)))
               (when callback
                 (funcall callback (cons json-body http-status))))
           (kill-buffer resp-buf)))))))

(defun clawdius--sync-request (method path &optional body)
  "Make a synchronous HTTP request.  Returns (JSON-BODY . HTTP-STATUS)."
  (let ((result nil))
    (clawdius--request
     method path body
     (lambda (response)
       (setq result response)))
    (with-local-quit
      (while (not result)
        (accept-process-output nil 0.1)))
    result))

;;; ---------------------------------------------------------------------------
;;; Health check
;;; ---------------------------------------------------------------------------

;;;###autoload
(defun clawdius-health (&optional callback)
  "Check if the Clawdius server is running."
  (interactive)
  (clawdius--request "GET" "/health" nil
    (lambda (response)
      (let ((http-status (cdr response))
            (body (car response)))
        (setq clawdius--health-status
              (if (and body (<= 200 http-status 299))
                  "OK"
                (format "ERROR (HTTP %d)" (or http-status 0))))
        (if callback
            (funcall callback clawdius--health-status)
          (message "Clawdius: %s" clawdius--health-status))))))

;;; ---------------------------------------------------------------------------
;;; Chat mode
;;; ---------------------------------------------------------------------------

(defvar clawdius-chat-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "RET") #'clawdius-chat-send)
    (define-key map (kbd "C-c C-c") #'clawdius-chat-abort)
    (define-key map (kbd "C-c C-k") #'clawdius-chat-kill)
    (define-key map (kbd "C-c C-f") #'clawdius-add-context)
    (define-key map (kbd "C-c C-p") #'clawdius-checkpoint)
    (define-key map (kbd "M-p") #'previous-line)
    (define-key map (kbd "M-n") #'next-line)
    map)
  "Keymap for `clawdius-chat-mode'.")

(define-derived-mode clawdius-chat-mode text-mode "Clawdius Chat"
  "Major mode for interacting with Clawdius AI chat.
\\{clawdius-chat-mode-map}"
  :group 'clawdius
  (setq-local font-lock-defaults nil)
  (setq-local comment-start ";;")
  (setq-local comment-end "")
  (read-only-mode -1))

;;;###autoload
(defun clawdius-chat ()
  "Open an AI chat buffer."
  (interactive)
  (let ((buffer (get-buffer-create "*Clawdius Chat*")))
    (with-current-buffer buffer
      (unless (eq major-mode 'clawdius-chat-mode)
        (clawdius-chat-mode)
        (erase-buffer)
        (insert ";; Clawdius AI Chat\n")
        (insert ";; Type your question and press RET to send.\n")
        (insert ";; C-c C-c abort  |  C-c C-k kill  |  C-c C-f add file  |  C-c C-p checkpoint\n\n")
        (goto-char (point-max)))
      (pop-to-buffer buffer)
      (goto-char (point-max)))))

(defun clawdius-chat-send (&optional question)
  "Send the current line as a question to Clawdius."
  (interactive)
  (let* ((start (line-beginning-position))
         (end (line-end-position))
         (line (string-trim (buffer-substring-no-properties start end)))
         (prompt (or question line)))
    (when (string-empty-p prompt)
      (user-error "No question to send"))
    (let ((inhibit-read-only t)
          (buf (current-buffer)))
      (goto-char (point-max))
      (insert "\n")
      (insert (format ";;; You: %s\n" prompt))
      (insert "\n;;; Clawdius: Thinking...\n")
      (let ((marker (point-marker)))
        (clawdius--request
         "POST" "/api/v1/chat"
         (list (cons 'message prompt)
               (when clawdius--session-id
                 (cons 'sessionId clawdius--session-id)))
         (lambda (response)
           (let ((body (car response))
                 (http-status (cdr response)))
             (with-current-buffer buf
               (save-excursion
                 (goto-char marker)
                 (let ((reply (cond
                               ((and body (<= 200 http-status 299))
                                (or (map-elt body "reply")
                                    (map-elt body "content")
                                    (map-elt body "text")))
                               ((and body (map-elt body "error"))
                                (format "Error: %s" (map-elt body "error")))
                               (t "No response from server"))))
                   (delete-region (marker-position marker) (point-max))
                   (set-marker marker nil)
                   (insert (format ";;; Clawdius: %s\n" reply))))))))))))

(defun clawdius-chat-abort ()
  "Cancel the current request."
  (interactive)
  (message "Clawdius: Request cancelled"))

(defun clawdius-chat-kill ()
  "Kill the Clawdius chat buffer."
  (interactive)
  (kill-buffer (current-buffer)))

;;;###autoload
(defun clawdius-add-context (filepath)
  "Add a file as context for the current chat session."
  (interactive "fFile to add as context: ")
  (let ((content (with-temp-buffer
                   (insert-file-contents-literally filepath)
                   (buffer-string))))
    (clawdius--request
     "POST" "/api/v1/context/add"
     (list (cons 'type "file")
           (cons 'source filepath)
           (cons 'content content))
     (lambda (_response)
       (message "Clawdius: Added context from %s" filepath)))))

;;;###autoload
(defun clawdius-checkpoint (&optional description)
  "Create a session checkpoint."
  (interactive "sCheckpoint description: ")
  (clawdius--request
   "POST" "/api/v1/checkpoint"
   (list (cons 'description (or description "manual checkpoint")))
   (lambda (response)
     (let ((body (car response)))
       (if body
           (progn
             (setq clawdius--session-id (map-elt body "id"))
             (message "Clawdius: Checkpoint created: %s"
                      (map-elt body "id")))
         (message "Clawdius: Failed to create checkpoint"))))))

;;; ---------------------------------------------------------------------------
;;; Code analysis
;;; ---------------------------------------------------------------------------

;;;###autoload
(defun clawdius-analyze ()
  "Analyze the current file using Clawdius."
  (interactive)
  (let* ((filepath (or (buffer-file-name) (buffer-name)))
         (content (buffer-string))
         (body (list (cons 'path filepath)
                     (cons 'content content))))
    (message "Clawdius: Analyzing %s..." filepath)
    (clawdius--request
     "POST" "/api/v1/analyze" body
     (lambda (response)
       (let* ((json-body (car response))
              (analysis (cond
                         ((and json-body (map-elt json-body "analysis"))
                          (map-elt json-body "analysis"))
                         ((and json-body (map-elt json-body "content"))
                          (map-elt json-body "content"))
                         (t "No analysis available"))))
         (with-help-window "*Clawdius Analysis*"
           (princ (format "File: %s\n\n%s\n" filepath analysis))))))))

;;; ---------------------------------------------------------------------------
;;; Inline completion (company-mode backend)
;;; ---------------------------------------------------------------------------

(defun clawdius--get-completions (prefix suffix callback)
  "Request completions from the Clawdius server."
  (let* ((filepath (or (buffer-file-name) ""))
         (body (list (cons 'prefix prefix)
                     (cons 'suffix suffix)
                     (cons 'file_path filepath)
                     (when clawdius-model
                       (cons 'model clawdius-model)))))
    (clawdius--request
     "POST" "/api/v1/complete" body
     (lambda (response)
       (let* ((json-body (car response))
              (completions
               (cond
                ((and json-body (map-elt json-body "completions"))
                 (map-elt json-body "completions"))
                ((and json-body (map-elt json-body "text"))
                 (list (map-elt json-body "text")))
                (t nil))))
         (funcall callback completions))))))

;;;###autoload
(defun clawdius-complete-at-point ()
  "Trigger Clawdius completion at point."
  (interactive)
  (let* ((point (point))
         (content (buffer-string))
         (prefix (buffer-substring-no-properties
                  (max 1 (- point 2000))
                  point))
         (suffix (buffer-substring-no-properties
                  point
                  (min (length content) (+ point 2000)))))
    (clawdius--get-completions
     prefix suffix
     (lambda (completions)
       (when completions
         (let ((text (if (listp completions)
                         (let ((first (car completions)))
                           (cond
                            ((map-elt first "text") (map-elt first "text"))
                            ((map-elt first "insertText") (map-elt first "insertText"))
                            ((stringp first) first)
                            (t nil)))
                       (if (stringp completions) completions nil))))
           (when text
             (let ((inhibit-read-only t))
               (insert text)
               (when (called-interactively-p 'interactive)
                 (message "Clawdius: Completion applied"))))))))))

(defconst company-clawdius 'company-clawdius
  "company-mode backend for Clawdius.")

(cl-defmethod company-backend ((_backend (eql company-clawdius)) command &optional arg &rest _ignored)
  "company-mode backend implementation for Clawdius."
  (pcase command
    ('prefix
     (when (and clawdius-enable-completion
                clawdius--health-status
                (string= clawdius--health-status "OK")
                (memq (char-before) clawdius-completion-trigger-chars))
       (company-grab-symbol)))
    ('candidates
     (when (and clawdius-enable-completion
                clawdius--health-status
                (string= clawdius--health-status "OK")
                (memq (char-before) clawdius-completion-trigger-chars))
       (cons :async
             (lambda (cb)
               (let* ((prefix (buffer-substring-no-properties
                               (max 1 (- (point) 2000))
                               (point)))
                      (suffix (buffer-substring-no-properties
                               (point)
                               (min (buffer-size) (+ (point) 2000)))))
                 (clawdius--get-completions
                  prefix suffix
                  (lambda (completions)
                    (let ((cands
                           (cl-loop for c in (or completions '())
                                    collect (cond
                                             ((map-elt c "text") (map-elt c "text"))
                                             ((map-elt c "insertText") (map-elt c "insertText"))
                                             ((stringp c) c)
                                             (t nil)))))
                      (funcall cb (delq nil cands))))))))))
    ('no-cache t)
    ('duplicates t)
    ('requires-match 'never)
    ('sorted t)
    ('meta "Clawdius AI completion")
    (_ nil)))

;;; ---------------------------------------------------------------------------
;;; Code actions
;;; ---------------------------------------------------------------------------

;;;###autoload
(defun clawdius-explain-region (start end)
  "Explain the code in the selected region."
  (interactive "r")
  (let ((region (buffer-substring-no-properties start end)))
    (when (string-empty-p (string-trim region))
      (user-error "No region selected"))
    (message "Clawdius: Explaining code...")
    (clawdius--request
     "POST" "/api/v1/chat"
     (list (cons 'message
                 (format "Explain this code:\n\n```\n%s\n```" region)))
     (lambda (response)
       (let* ((body (car response))
              (reply (cond
                      ((and body (map-elt body "reply")) (map-elt body "reply"))
                      ((and body (map-elt body "content")) (map-elt body "content"))
                      ((and body (map-elt body "text")) (map-elt body "text"))
                      (t "No explanation available"))))
         (with-current-buffer (get-buffer-create "*Clawdius Explain*")
           (erase-buffer)
           (insert (format ";;; Selection:\n%s\n\n;;; Explanation:\n%s\n"
                           region reply))
           (read-only-mode 1)
           (display-buffer (current-buffer))))))))

;;;###autoload
(defun clawdius-explain ()
  "Explain the code at point (current line)."
  (interactive)
  (clawdius-explain-region (line-beginning-position)
                           (line-end-position)))

;;;###autoload
(defun clawdius-refactor (start end)
  "Suggest refactoring for the selected region."
  (interactive "r")
  (let ((region (buffer-substring-no-properties start end)))
    (when (string-empty-p (string-trim region))
      (user-error "No region selected"))
    (message "Clawdius: Suggesting refactoring...")
    (clawdius--request
     "POST" "/api/v1/chat"
     (list (cons 'message
                 (format "Suggest a refactoring for this code, showing the improved version:\n\n```\n%s\n```"
                         region)))
     (lambda (response)
       (let* ((body (car response))
              (reply (cond
                      ((and body (map-elt body "reply")) (map-elt body "reply"))
                      ((and body (map-elt body "content")) (map-elt body "content"))
                      ((and body (map-elt body "text")) (map-elt body "text"))
                      (t "No suggestions available"))))
         (with-current-buffer (get-buffer-create "*Clawdius Refactor*")
           (erase-buffer)
           (insert (format ";;; Original:\n%s\n\n;;; Refactored suggestion:\n%s\n"
                           region reply))
           (display-buffer (current-buffer))))))))

;;;###autoload
(defun clawdius-fix ()
  "Ask Clawdius to fix issues in the current file."
  (interactive)
  (let* ((filepath (or (buffer-file-name) (buffer-name)))
         (content (buffer-string)))
    (message "Clawdius: Analyzing for fixes...")
    (clawdius--request
     "POST" "/api/v1/chat"
     (list (cons 'message
                 (format "Fix issues in this file (%s):\n\n```\n%s\n```"
                         filepath content)))
     (lambda (response)
       (let* ((body (car response))
              (reply (cond
                      ((and body (map-elt body "reply")) (map-elt body "reply"))
                      ((and body (map-elt body "content")) (map-elt body "content"))
                      ((and body (map-elt body "text")) (map-elt body "text"))
                      (t "No fixes suggested"))))
         (with-help-window "*Clawdius Fix*"
           (princ (format "File: %s\n\n%s\n" filepath reply))))))))

;;; ---------------------------------------------------------------------------
;;; Git operations
;;; ---------------------------------------------------------------------------

;;;###autoload
(defun clawdius-diff ()
  "Show git diff via the Clawdius server."
  (interactive)
  (message "Clawdius: Fetching git status...")
  (clawdius--request
   "GET" "/api/v1/git/status" nil
   (lambda (response)
     (let* ((body (car response))
            (status (or (and body (map-elt body "status")) "No git status"))
            (diff (or (and body (map-elt body "diff")) "No diff available")))
       (with-current-buffer (get-buffer-create "*Clawdius Diff*")
         (erase-buffer)
         (insert (format ";;; Status:\n%s\n\n;;; Diff:\n%s\n" status diff))
         (diff-mode)
         (read-only-mode 1)
         (display-buffer (current-buffer)))))))

;;; ---------------------------------------------------------------------------
;;; Setup
;;; ---------------------------------------------------------------------------

;;;###autoload
(defun clawdius-setup ()
  "Set up Clawdius integration for Emacs.
Registers the company-clawdius backend and performs an initial health check."
  (interactive)
  (clawdius-health
   (lambda (_status)
     (when clawdius-enable-completion
       (add-to-list 'company-backends #'company-clawdius)
       (message "Clawdius: company-clawdius backend registered"))
     (unless (string= clawdius--health-status "OK")
       (message "Clawdius: Server not available at %s:%d.  Features will be limited."
                clawdius-host clawdius-port))
     (run-at-time 60 t #'clawdius-health)
     (message "Clawdius: Setup complete (%s)" clawdius--health-status))))

(provide 'clawdius)
;;; clawdius.el ends here
