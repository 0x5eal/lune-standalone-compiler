local net = require("@lune/net")
local process = require("@lune/process")
local stdio = require("@lune/stdio")
local task = require("@lune/task")

local PORT = 8080
local URL = `http://127.0.0.1:{PORT}`
local RESPONSE = "Hello, lune!"

-- Serve should not block the thread from continuing

local thread = task.delay(1, function()
	stdio.ewrite("Serve must not block the current thread\n")
	task.wait(1)
	process.exit(1)
end)

local handle = net.serve(PORT, function(request)
	-- print("Request:", request)
	-- print("Responding with", RESPONSE)
	assert(request.path == "/some/path")
	assert(request.query.key == "param2")
	assert(request.query.key2 == "param3")
	return RESPONSE
end)

task.cancel(thread)

-- Serve should respond to a request we send to it

local thread2 = task.delay(1, function()
	stdio.ewrite("Serve should respond to requests in a reasonable amount of time\n")
	task.wait(1)
	process.exit(1)
end)

local response = net.request(URL .. "/some/path?key=param1&key=param2&key2=param3").body
assert(response == RESPONSE, "Invalid response from server")

task.cancel(thread2)

-- Stopping is not guaranteed to happen instantly since it is async, but
-- it should happen on the next yield, so we wait the minimum amount here
handle.stop()
task.wait()

-- Sending a net request may error if there was
-- a connection issue, we should handle that here
local success, response2 = pcall(net.request, URL)
if not success then
	local message = tostring(response2)
	assert(
		string.find(message, "Connection reset")
			or string.find(message, "Connection closed")
			or string.find(message, "Connection refused")
			or string.find(message, "No connection could be made"), -- Windows Request Error
		"Server did not stop responding to requests"
	)
else
	assert(not response2.ok, "Server did not stop responding to requests")
end

--[[
	Trying to stop the server again should error and
	mention that the server has already been stopped

	Note that we cast pcall to any because of a
	Luau limitation where it throws a type error for
	`err` because handle.stop doesn't return any value
]]
local success2, err = (pcall :: any)(handle.stop)
assert(not success2, "Calling stop twice on the net serve handle should error")
local message = tostring(err)
assert(
	string.find(message, "stop")
		or string.find(message, "shutdown")
		or string.find(message, "shut down"),
	"The error message for calling stop twice on the net serve handle should be descriptive"
)
