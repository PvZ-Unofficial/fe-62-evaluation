import copy
import unittest
from evaluate import validate


class CompletenessTests(unittest.TestCase):
    def setUp(self):
        self.root = dict(seed=1234567, n=1, status="failed", cause="home", sun=99925)
        self.rounds = {1234567: [dict(round=1, outcome="success", end_sun=99950),
                                dict(round=2, outcome="home", end_sun=99925)]}

    def test_complete_chain(self):
        validate([self.root], self.rounds, [1234567])

    def test_censored_chain_rejected(self):
        with self.assertRaises(ValueError):
            validate([dict(self.root, status="censored")], self.rounds, [1234567])

    def test_missing_or_duplicate_round_rejected(self):
        for chain in [self.rounds[1234567][1:], self.rounds[1234567] * 2]:
            with self.assertRaises(ValueError):
                validate([self.root], {1234567: chain}, [1234567])

    def test_duplicate_root_rejected(self):
        with self.assertRaises(ValueError):
            validate([self.root, self.root], self.rounds, [1234567, 1234568])

    def test_wrong_balance_rejected(self):
        with self.assertRaises(ValueError):
            validate([dict(self.root, sun=100000)], self.rounds, [1234567])


if __name__ == "__main__":
    unittest.main()
