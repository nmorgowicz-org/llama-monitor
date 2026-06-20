# Quick Start

Get your first local AI model up and running with Llama Monitor.

## 1. Launch Llama Monitor

Open Llama Monitor. You’ll land on the Setup screen, where you can browse available models, see your hardware memory, and get started.

## 2. Use the Setup wizard to pick a model

Click **Open setup wizard** on the Setup screen.

The Setup wizard walks you through:

- Choosing a model from the list (curated picks and Hugging Face search).
- Picking a quant (e.g., Q4_K_M for a good balance of quality and speed).
- Setting hardware and memory (VRAM estimator) to keep your system stable.

## 3. Adjust hardware / memory (VRAM estimator)

On the Hardware & memory step:

- Use the suggested GPU layers (usually "Auto (recommended)").
- Check the VRAM estimator bar: it shows:
  - How much memory your model will use.
  - Whether you’re under budget (green), tight (yellow), or at risk of OOM (red).
- If it says "At risk", reduce GPU layers or context size until it shows "Safe".

## 4. Start server

At the end of the wizard:

- Review your preset settings.
- Click **Start server**.

Llama Monitor will:

- Launch llama-server with your chosen preset.
- Switch you to the Performance & metrics dashboard.

## 5. Open a new conversation

- In the sidebar, click the **Chat** tab.
- Use **New conversation** to start a chat with your model.
- Send a message; you’ll see live tokens/sec, context usage, and response quality.

## 6. Use Performance & metrics to check health

On the **Server** page (Performance & metrics), watch:

- **Speed (throughput)** – tokens/sec (both prompt and generation).
- **Active sessions** – current parallel chat sessions.
- **Requests** – how many requests you’ve sent.
- **Model info** – current model, quant, and speculative decoding status.
- **GPU / System** – memory and load if you have a remote agent (optional but recommended).
- **Memory pressure** (macOS) – shows free memory, compressed memory, and swap activity. A warning or critical indicator means macOS is struggling; reduce context, stop heavy downloads, or disable mlock.

If you see:

- Steady tokens/sec and no red warnings: your setup is healthy.
- Very low tokens/sec or memory warnings: return to the preset and reduce GPU layers or context size.
- Memory pressure at warning or critical: macOS is under pressure; lower context size or free memory before your system becomes unresponsive.

That’s it—you’re running your local AI model with full visibility.

## 7. Use sleep modes when the server is running

The monitoring chip in the top nav lets you cycle through three modes:

- **Monitoring** – full telemetry, GPU reads, system metrics, and live logs.
- **Logs only** – only the live log stream is active; GPU, system, and sparkline updates are paused to save resources.
- **Paused** – all telemetry and logs are paused; llama-server keeps running.

Click the chip to cycle modes, or use it when you want the server running but need lower overhead on your system.
