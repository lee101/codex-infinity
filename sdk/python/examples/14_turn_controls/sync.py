import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from codex_app_server import Codex, TextInput

with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
    steer_turn = thread.turn(TextInput("Count from 1 to 40 with commas, then one summary sentence."))
    steer_result = "sent"
    try:
        _ = steer_turn.steer(TextInput("Keep it brief and stop after 10 numbers."))
    except Exception as exc:
        steer_result = f"skipped {type(exc).__name__}"

    steer_event_count = 0
    steer_completed_status = None
    steer_deltas = []
    for event in steer_turn.stream():
        steer_event_count += 1
        if event.method == "item/agentMessage/delta":
            steer_deltas.append(event.payload.delta)
            continue
        if event.method == "turn/completed":
            steer_completed_turn = event.payload.turn
            steer_completed_status = getattr(event.payload.turn.status, "value", str(event.payload.turn.status))

    if steer_completed_status is None:
        raise RuntimeError("stream ended without turn/completed")
    steer_preview = "".join(steer_deltas).strip()

    interrupt_turn = thread.turn(TextInput("Count from 1 to 200 with commas, then one summary sentence."))
    interrupt_result = "sent"
    try:
        _ = interrupt_turn.interrupt()
    except Exception as exc:
        interrupt_result = f"skipped {type(exc).__name__}"

    interrupt_event_count = 0
    interrupt_completed_status = None
    interrupt_deltas = []
    for event in interrupt_turn.stream():
        interrupt_event_count += 1
        if event.method == "item/agentMessage/delta":
            interrupt_deltas.append(event.payload.delta)
            continue
        if event.method == "turn/completed":
            interrupt_completed_turn = event.payload.turn
            interrupt_completed_status = getattr(event.payload.turn.status, "value", str(event.payload.turn.status))

    interrupt_preview = assistant_text_from_turn(interrupt_completed_turn).strip() or "[no assistant text]"

    print("steer.result:", steer_result.model_dump(mode="json", by_alias=True))
    print("steer.final.status:", steer_completed_status)
    print("steer.events.count:", steer_event_count)
    print("steer.assistant.preview:", steer_preview)
    print("interrupt.result:", interrupt_result.model_dump(mode="json", by_alias=True))
    print("interrupt.final.status:", interrupt_completed_status)
    print("interrupt.events.count:", interrupt_event_count)
    print("interrupt.assistant.preview:", interrupt_preview)
