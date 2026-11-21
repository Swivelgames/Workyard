import assert from "node:assert/strict";
import { createRequire } from "node:module";

const requireSdk = createRequire(new URL("../target/mcp-interop/package.json", import.meta.url));
const { StdioClientTransport } = requireSdk("@modelcontextprotocol/sdk/client/stdio.js");

// The smoke client makes one request at a time. Compare the whole session so
// a progress frame arriving after its result (even during the next call) counts.
export class ProgressTransport extends StdioClientTransport {
	#expected = [];
	#received = [];
	#nextMessages;

	expectProgress(messages) {
		assert.equal(this.#nextMessages, undefined, "Previous progress expectation was unused");
		this.#nextMessages = messages;
	}

	send(request, options) {
		if (request.method === "tools/call" && this.#nextMessages !== undefined) {
			const token = request.params._meta?.progressToken;
			assert.notEqual(token, undefined, "Expected the SDK to subscribe to progress");
			for (const [index, message] of this.#nextMessages.entries()) {
				this.#expected.push({ progressToken: token, progress: index + 1, message, total: undefined });
			}
			this.#nextMessages = undefined;
		}

		if ("id" in request) this.#expected.push({ reply: request.id });
		return super.send(request, options);
	}

	// The SDK preserves the transport's existing listener when it connects.
	onmessage = frame => {
		if (frame.method === "notifications/progress") {
			const { progressToken, progress, message, total } = frame.params ?? {};
			this.#received.push({ progressToken, progress, message, total });
		} else if ("result" in frame || "error" in frame) {
			this.#received.push({ reply: frame.id });
		}
	};

	verify() {
		assert.equal(this.#nextMessages, undefined, "Expected a subscribed tool call");
		assert.deepEqual(this.#received, this.#expected, "MCP progress frames and replies in arrival order");
	}
}

// Give the SDK callback its own check against real stdio. SDK 1.32.2 defers
// notification handlers to a microtask but processes a tool result synchronously.
export class CallbackTransport extends StdioClientTransport {
	start() {
		const deliver = this.onmessage;
		let delivered = Promise.resolve();
		this.onmessage = (frame, extra) => {
			delivered = delivered.then(() => deliver(frame, extra));
		};
		return super.start();
	}
}
