"""utau-edit-cost 17: who may read holdout4, and what each tool logs (read.py's `ledger_policy`, `missing_texts`,
`Ledger` and `rule3_applies`).

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


def test_the_lyric_read_waits_for_the_edit_cost_read():
    refused(R.ledger_policy("lyric", True, False, 0, N, N))
    refused(R.ledger_policy("lyric", True, False, 1, 3, N))  # step 1 crashed: its alignment is not settled
    assert R.ledger_policy("lyric", True, False, 3, 0, N) == ([], "3")


def test_the_lyric_read_runs_once_and_resumes_only_after_a_failed_lyric_read():
    refused(R.ledger_policy("lyric", True, False, 5, 0, N, prior_lyric_reads=1))
    refused(R.ledger_policy("lyric", True, True, 3, 0, N, prior_lyric_reads=0))
    assert R.ledger_policy("lyric", True, True, 5, 0, N, prior_lyric_reads=2) == ([], "3")


def test_the_lyric_read_needs_a_text_for_every_aligned_song():
    have = {"a", "b"}
    assert R.missing_texts(["a", "b"], have.__contains__) == []
    errs = R.missing_texts(["a", "b", "c", "d"], have.__contains__)
    assert len(errs) == 1 and "2 of 4" in errs[0] and "c, d" in errs[0], errs


def test_the_frame_read_reuses_step_one_and_never_resumes():
    refused(R.ledger_policy("frame", True, False, 0, N, N))
    refused(R.ledger_policy("frame", True, False, 1, 2, N))
    refused(R.ledger_policy("frame", True, True, 1, 0, N))
    assert R.ledger_policy("frame", True, False, 1, 0, N) == ([], "2")


def test_dev_is_never_refused():
    assert R.ledger_policy("edit", False, False, 0, 0, 10) == ([], "1")
    assert R.ledger_policy("edit", False, True, 0, 0, 10) == ([], "3")
    assert R.ledger_policy("lyric", False, False, 0, 0, 10) == ([], "3")
    assert R.ledger_policy("frame", False, False, 0, 0, 10) == ([], "2")


def test_only_the_first_tool_of_a_fresh_read_logs_a_first_read():
    led = R.Ledger(True, "why", "edit", False)
    assert led.args("alignment check") == ["--reason", "why"]
    assert led.args("stage D") == ["--reason", "why (same sitting: stage D)", "--reread"]


def test_a_resumed_read_logs_every_tool_as_a_resumed_reread():
    led = R.Ledger(True, "why", "edit", True)
    assert led.args("stage D") == ["--reason", "why (resumed after a failed run: stage D)", "--reread"]
    assert led.args("stage C")[-1] == "--reread"


def test_the_lyric_read_logs_rereads_naming_rule_two():
    led = R.Ledger(True, "why", "lyric", False)
    assert led.args("stage D") == ["--reason", "why (rule 2, the lyric read: stage D)", "--reread"]
    assert led.args("stage C") == ["--reason", "why (rule 2, the lyric read: stage C)", "--reread"]
    resumed = R.Ledger(True, "why", "lyric", True).args("stage D")
    assert resumed == ["--reason", "why (rule 2, the lyric read, resumed after a failed run: stage D)", "--reread"]
    assert R.LYRIC_READ in resumed[1]  # a resumed lyric read still counts as a lyric read


def test_the_frame_read_is_its_own_logged_reread():
    led = R.Ledger(True, "why", "frame", False)
    assert led.args("bar 1") == ["--reason", "why (rule 4, bar 1: the frame read: bar 1)", "--reread"]


def test_dev_logs_nothing():
    assert R.Ledger(False, None, "edit", False).args("stage D") == []
    assert R.Ledger(False, None, "lyric", False).args("stage D") == []


def test_rule_three_ships_t_only_if_rule_one_passes_and_rule_two_fails():
    assert R.rule3_applies(True, False, True) == "T ships on C"
    assert R.rule3_applies(True, True, True).startswith("T stays opt-in")
    assert R.rule3_applies(False, False, True).startswith("T does not ship")
    assert R.rule3_applies(True, False, False).startswith("T stays opt-in")


if __name__ == "__main__":
    tests = [(k, v) for k, v in sorted(globals().items()) if k.startswith("test_")]
    for name, t in tests:
        t()
        print("ok", name)
    print(f"{len(tests)} passed")
