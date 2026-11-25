"""utau-edit-cost 17: who may read holdout4, and what each tool logs (read.py's `ledger_policy` and `Ledger`).

Run: python3 -I -B test_read.py   (exits non-zero on the first failure; no corpus data needed)
"""
import sys

sys.dont_write_bytecode = True
import os  # noqa: E402

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import read as R  # noqa: E402

N = 24


def refused(errs):
    assert errs and errs[1] is None, errs


def test_the_edit_cost_read_starts_once_on_an_unread_split():
    assert R.ledger_policy("edit", True, False, 0, N, N) == ([], "1")
    refused(R.ledger_policy("edit", True, False, 1, 0, N))
    refused(R.ledger_policy("edit", True, False, 0, N - 3, N))  # statuses settled without a read: not the split's manifest


def test_resume_needs_a_logged_read_and_restarts_where_the_crash_left_off():
    refused(R.ledger_policy("edit", True, True, 0, N, N))
    assert R.ledger_policy("edit", True, True, 1, N, N) == ([], "1")  # crashed inside step 1: check again
    assert R.ledger_policy("edit", True, True, 1, 0, N) == ([], "3")  # step 1 finished: reuse it


def test_the_frame_read_reuses_step_one_and_never_resumes():
    refused(R.ledger_policy("frame", True, False, 0, N, N))
    refused(R.ledger_policy("frame", True, False, 1, 2, N))
    refused(R.ledger_policy("frame", True, True, 1, 0, N))
    assert R.ledger_policy("frame", True, False, 1, 0, N) == ([], "2")


def test_dev_is_never_refused():
    assert R.ledger_policy("edit", False, False, 0, 0, 10) == ([], "1")
    assert R.ledger_policy("edit", False, True, 0, 0, 10) == ([], "3")
    assert R.ledger_policy("frame", False, False, 0, 0, 10) == ([], "2")


def test_only_the_first_tool_of_a_fresh_read_logs_a_first_read():
    led = R.Ledger(True, "why", "edit", False)
    assert led.args("alignment check") == ["--reason", "why"]
    assert led.args("stage D") == ["--reason", "why (same sitting: stage D)", "--reread"]


def test_a_resumed_read_logs_every_tool_as_a_resumed_reread():
    led = R.Ledger(True, "why", "edit", True)
    assert led.args("stage D") == ["--reason", "why (resumed after a failed run: stage D)", "--reread"]
    assert led.args("stage C")[-1] == "--reread"


def test_the_frame_read_is_its_own_logged_reread():
    led = R.Ledger(True, "why", "frame", False)
    assert led.args("bar 1") == ["--reason", "why (rule 4, bar 1: the frame read: bar 1)", "--reread"]


def test_dev_logs_nothing():
    assert R.Ledger(False, None, "edit", False).args("stage D") == []


if __name__ == "__main__":
    tests = [(k, v) for k, v in sorted(globals().items()) if k.startswith("test_")]
    for name, t in tests:
        t()
        print("ok", name)
    print(f"{len(tests)} passed")
