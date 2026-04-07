--- Clawdius Neovim Plugin
--- Provides AI-powered coding assistance via the Clawdius server
---
--- Usage:
---   require('clawdius').setup({
---     host = 'localhost',
---     port = 8080,
---     api_key = nil,  -- or from env CLAWDIUS_API_KEY
---     enabled = true,
---   })

local M = {}
local config = {
    host = 'localhost',
    port = 8080,
    api_key = nil,
    enabled = true,
    timeout = 10000,
}

local function http_request(method, path, body)
    local url = string.format('http://%s:%d%s', config.host, config.port, path)
    local cmd = { 'curl', '-s', '-X', method, url, '-H', 'Content-Type: application/json' }

    if config.api_key then
        table.insert(cmd, '-H')
        table.insert(cmd, 'Authorization: Bearer ' .. config.api_key)
    end

    if body then
        table.insert(cmd, '-d')
        table.insert(cmd, vim.json.encode(body))
    end

    local result = vim.fn.system(cmd)
    local ok, decoded = pcall(vim.json.decode, result)
    if not ok then
        return nil, 'Failed to parse response from Clawdius server'
    end
    return decoded
end

function M.health()
    local ok, response = pcall(http_request, 'GET', '/health')
    if not ok or not response then
        return false, 'Cannot connect to Clawdius server'
    end
    return true, 'Clawdius server is running'
end

function M.complete(line, col, callback)
    local buf = vim.api.nvim_get_current_buf()
    local filepath = vim.api.nvim_buf_get_name(buf)
    local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)

    local body = {
        path = filepath,
        line = line,
        column = col,
        context = table.concat(lines, '\n'),
    }

    local ok, response = pcall(http_request, 'POST', '/api/v1/complete', body)
    if ok and response and response.completions then
        callback(response.completions)
    else
        callback({})
    end
end

function M.chat(question, callback)
    local buf = vim.api.nvim_get_current_buf()
    local filepath = vim.api.nvim_buf_get_name(buf)
    local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)

    local body = {
        message = question,
        context = {
            file = filepath,
            content = table.concat(lines, '\n'),
        },
    }

    local ok, response = pcall(http_request, 'POST', '/api/v1/chat', body)
    if ok and response and response.reply then
        callback(response.reply)
    else
        callback('Error: Failed to get response from Clawdius')
    end
end

function M.analyze(callback)
    local buf = vim.api.nvim_get_current_buf()
    local filepath = vim.api.nvim_buf_get_name(buf)
    local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)

    local body = {
        path = filepath,
        content = table.concat(lines, '\n'),
    }

    local ok, response = pcall(http_request, 'POST', '/api/v1/analyze', body)
    if ok and response and response.analysis then
        callback(response.analysis)
    else
        callback('Error: Failed to analyze code')
    end
end

function M.git_status(callback)
    local ok, response = pcall(http_request, 'GET', '/api/v1/git/status')
    if ok and response then
        callback(response)
    else
        callback({ error = 'Failed to get git status' })
    end
end

function M.setup(opts)
    if opts then
        for k, v in pairs(opts) do
            config[k] = v
        end
    end

    if config.api_key == nil then
        config.api_key = vim.env.CLAWDIUS_API_KEY
    end

    vim.defer_fn(function()
        local healthy, msg = M.health()
        if not healthy then
            vim.notify('Clawdius: ' .. msg, vim.log.levels.WARN)
        end
    end, 1000)

    pcall(function()
        local has_cmp, cmp = pcall(require, 'cmp')
        if has_cmp then
            cmp.register_source('clawdius', {
                documentation = true,
                trigger_characters = { '.', ':', '(' },
                complete = function(params, callback)
                    M.complete(params.context.cursor.line, params.context.cursor.col, callback)
                end,
            })
        end
    end)
end

function M.open_chat()
    local width = math.min(80, math.floor(vim.o.columns * 0.8))
    local height = math.min(20, math.floor(vim.o.lines * 0.5))
    local col = math.floor((vim.o.columns - width) / 2)
    local row = math.floor((vim.o.lines - height) / 2)

    local buf = vim.api.nvim_create_buf(false, true)
    local win = vim.api.nvim_open_win(buf, true, {
        relative = 'editor',
        width = width,
        height = height,
        col = col,
        row = row,
        style = 'minimal',
        border = 'rounded',
        title = ' Clawdius Chat ',
        title_pos = 'center',
    })

    vim.api.nvim_buf_set_keymap(buf, 'n', '<CR>', '', {
        callback = function()
            local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
            local question = table.concat(lines, '\n')
            if question:match('^%s*$') then return end

            vim.api.nvim_buf_set_lines(buf, 0, -1, false, { 'Thinking...' })
            M.chat(question, function(reply)
                vim.schedule(function()
                    vim.api.nvim_buf_set_lines(buf, 0, -1, false, vim.split(reply, '\n'))
                end)
            end)
        end,
    })

    vim.api.nvim_buf_set_keymap(buf, 'n', 'q', '', {
        callback = function()
            vim.api.nvim_win_close(win, true)
        end,
    })

    vim.api.nvim_set_current_buf(buf)
end

vim.api.nvim_create_user_command('ClawdiusChat', function()
    M.open_chat()
end, { desc = 'Open Clawdius AI chat' })

vim.api.nvim_create_user_command('ClawdiusAnalyze', function()
    local buf = vim.api.nvim_get_current_buf()
    M.analyze(function(result)
        vim.notify(result, vim.log.levels.INFO)
    end)
end, { desc = 'Analyze current file with Clawdius' })

vim.api.nvim_create_user_command('ClawdiusHealth', function()
    local healthy, msg = M.health()
    if healthy then
        vim.notify('Clawdius: OK - ' .. msg, vim.log.levels.INFO)
    else
        vim.notify('Clawdius: ERROR - ' .. msg, vim.log.levels.ERROR)
    end
end, { desc = 'Check Clawdius server health' })

return M
