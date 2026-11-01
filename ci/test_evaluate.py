import copy
import unittest
from evaluate import validate, shard_count, shard_seeds


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

class PartitionTests(unittest.TestCase):
    def test_old_requests_preserve_exact_six_partitions(self):
        req = {"seeds": list(range(300))}
        for i in range(6):
            self.assertEqual(shard_seeds(req, i), list(range(i*50, (i+1)*50)))

    def test_parallel_partitions_cover_every_root_exactly_once(self):
        for count in [1, 14, 30, 60, 300]:
            req = {"seeds": list(range(300)), "shards": count}
            chunks = [shard_seeds(req, i) for i in range(count)]
            self.assertEqual([s for chunk in chunks for s in chunk], req["seeds"])
            self.assertTrue(all(chunks))
            self.assertLessEqual(max(map(len,chunks))-min(map(len,chunks)),1)

    def test_invalid_partition_is_rejected(self):
        for count in [0, 301, True, "60"]:
            with self.assertRaises(ValueError):
                shard_count({"seeds": list(range(300)), "shards": count})
        for index in [-1, 60]:
            with self.assertRaises(ValueError):
                shard_seeds({"seeds": list(range(300)), "shards":60}, index)
