#pragma once

#include<cstring>
#include<cstddef>
#include<cassert>
#include<cstdint>
#include<array>
#include<algorithm>
#include<type_traits>
#include<utility>

namespace pvz_emulator::object {

template<typename T, size_t S> class obj_list {
	class obj_wrap {
	protected:
		T t;
		size_t next_available;
		std::uint32_t active_id;
		bool active;

		obj_wrap() :t(), next_available(0), active_id(0), active(false) {}
		friend class obj_list;
	};

	static_assert(S > 0);
	static_assert(S <= 0x10000);
	static_assert(std::is_standard_layout_v<obj_wrap>);
	static_assert(offsetof(obj_wrap, t) == 0);
	static_assert(std::is_same_v<decltype(std::declval<obj_wrap>().active_id), std::uint32_t>);
	static_assert(std::is_same_v<decltype(std::declval<obj_wrap>().active), bool>);

	class iterator {
	protected:
		int i;
		obj_list& list;

	public:
		explicit iterator(int n, obj_list& l) : i(n), list(l) {
			while (i >= 0 && static_cast<size_t>(i) < list.active_end &&
				(!list.a[static_cast<size_t>(i)].active || list.a[static_cast<size_t>(i)].t.is_freeable()))
			{
				i++;
			}
		}

		iterator& operator++() {
			do {
				i++;
			} while (i >= 0 && static_cast<size_t>(i) < list.active_end &&
				(!list.a[static_cast<size_t>(i)].active || list.a[static_cast<size_t>(i)].t.is_freeable()));
			return *this;
		}

		iterator operator++(int) {
			auto original = *this;
			++(*this);
			return original;
		}

		bool operator==(const iterator& other) const {
			if (&list != &other.list) {
				return false;
			}

			if (other.i == i) {
				return true;
			}
			else if (i == -1) {
				return other.i < 0 || static_cast<size_t>(other.i) >= list.active_end;
			}
			else if (other.i == -1) {
				return i < 0 || static_cast<size_t>(i) >= list.active_end;
			}
			else {
				return false;
			}
		}

		bool operator!=(const iterator& other) const {
			return !((*this) == other);
		}

		T& operator*() const {
			return list.a[static_cast<size_t>(i)].t;
		}
	};

	class const_iterator : public iterator {
	public:
		const_iterator(int i, const obj_list& l) :
			iterator(i, const_cast<obj_list&>(l)) {}

		const T& operator*() const {
			return this->list.a[static_cast<size_t>(this->i)].t;
		}
	};

	obj_wrap a[S];
	size_t next_available;
	size_t active_end;
	size_t max_used_end;
	size_t n_actives;
	unsigned int next_key;

	std::uint32_t make_id(size_t i) {
		unsigned int key = next_key++;
		if (next_key == 65536) {
			next_key = 1;
		}
		return (key << 16) | static_cast<unsigned int>(i);
	}

public:
	explicit obj_list() :
		next_available(0),
		active_end(0),
		max_used_end(0),
		n_actives(0),
		next_key(1001)
	{
	}

	T& alloc() {
		size_t i;

		if (next_available > max_used_end || max_used_end > S) {
			throw std::bad_alloc();
		}

		if (next_available >= max_used_end) {
			if (max_used_end < S) {
				i = max_used_end++;
				next_available = max_used_end;
			}
			else {
				throw std::bad_alloc();
			}
		}
		else {
			i = next_available;
			next_available = a[i].next_available;
		}

		active_end = std::max(active_end, i + 1);

		a[i].t = T{};
		a[i].active_id = make_id(i);
		a[i].active = true;
		++n_actives;

		return a[i].t;
	}

	T* get(int native_index) {
		if (native_index < 0 || static_cast<size_t>(native_index) >= max_used_end) {
			return nullptr;
		}

		auto& item = a[static_cast<size_t>(native_index)];
		return item.active ? &item.t : nullptr;
	}

	const T* get(int native_index) const {
		if (native_index < 0 || static_cast<size_t>(native_index) >= max_used_end) {
			return nullptr;
		}

		const auto& item = a[static_cast<size_t>(native_index)];
		return item.active ? &item.t : nullptr;
	}

	T* try_to_get(std::uint32_t id) {
		return const_cast<T*>(std::as_const(*this).try_to_get(id));
	}

	const T* try_to_get(std::uint32_t id) const {
		if (id == 0 || (id >> 16) == 0) {
			return nullptr;
		}

		const auto i = static_cast<size_t>(id & 0xffffu);
		if (i >= max_used_end || i >= S) {
			return nullptr;
		}

		const auto& item = a[i];
		return item.active && item.active_id == id ? &item.t : nullptr;
	}

	// Requires an object from an occupied slot of this obj_list specialization.
	// Same-frame death is allowed; reclamation, clear or destruction ends the borrow.
	static std::uint32_t get_id_unchecked(const T& object) noexcept {
		// obj_wrap is standard-layout and its first member is pointer-interconvertible.
		return reinterpret_cast<const obj_wrap*>(&object)->active_id;
	}

	std::uint32_t get_id(const T* p) const {
		if (p == nullptr) {
			return 0;
		}

		const auto address = reinterpret_cast<std::uintptr_t>(p);
		const auto begin = reinterpret_cast<std::uintptr_t>(&a[0]);
		const auto end = reinterpret_cast<std::uintptr_t>(&a[S]);
		if (address < begin || address >= end || (address - begin) % sizeof(obj_wrap) != 0) {
			return 0;
		}

		const auto& item = a[(address - begin) / sizeof(obj_wrap)];
		return item.active ? item.active_id : 0;
	}

	std::uint32_t get_id(const T& p) const {
		return get_id(&p);
	}

	int get_index(const T& p) const {
		auto id = get_id(p);
		return id == 0 ? -1 : static_cast<int>(id & 0xffffu);
	}

	bool is_active(const T* p) const {
		if (p == nullptr) {
			return false;
		}

		const auto address = reinterpret_cast<std::uintptr_t>(p);
		const auto begin = reinterpret_cast<std::uintptr_t>(&a[0]);
		const auto end = reinterpret_cast<std::uintptr_t>(&a[S]);
		if (address < begin || address >= end || (address - begin) % sizeof(obj_wrap) != 0) {
			return false;
		}

		return a[(address - begin) / sizeof(obj_wrap)].active;
	}

	void shrink_to_fit() {
		size_t last = 0;

		for (auto i = 0ul; i < active_end; i++) {
			if (!a[i].active) {
				continue;
			}

			if (a[i].t.is_freeable()) {
				a[i].active = false;
				a[i].active_id = 0;
				a[i].next_available = next_available;
				next_available = i;
				--n_actives;
			} else {
				last = i + 1;
			}
		}

		active_end = last;
		assert(next_available <= max_used_end);
	}


	[[nodiscard]]
	size_t size() const {
		size_t live = 0;
		for (auto it = begin(); it != end(); ++it) {
			++live;
		}
		return live;
	}

	[[nodiscard]]
	std::uint32_t max_used_count() const {
		assert(max_used_end <= 0x10000);
		return static_cast<std::uint32_t>(max_used_end);
	}

	[[nodiscard]]
	std::uint32_t active_count() const {
		assert(n_actives <= 0x10000);
		return static_cast<std::uint32_t>(n_actives);
	}

	static constexpr std::uint32_t capacity() {
		return static_cast<std::uint32_t>(S);
	}

	[[nodiscard]]
	std::uint32_t free_list_head() const {
		assert(next_available <= 0x10000);
		return static_cast<std::uint32_t>(next_available);
	}

	[[nodiscard]]
	std::uint32_t next_id_key() const {
		return next_key;
	}

	[[nodiscard]]
	bool slot_active(std::uint32_t index) const {
		return index < max_used_end && a[index].active;
	}

	[[nodiscard]]
	std::uint32_t slot_id(std::uint32_t index) const {
		return index < max_used_end ? a[index].active_id : 0;
	}

	[[nodiscard]]
	std::uint32_t slot_next_free(std::uint32_t index) const {
		assert(index < max_used_end && !a[index].active);
		assert(a[index].next_available <= 0x10000);
		return static_cast<std::uint32_t>(a[index].next_available);
	}

	iterator begin() {
		return iterator(0, *this);
	}

	iterator end() {
		return iterator(-1, *this);
	}

	const_iterator begin() const {
		return const_iterator(0, *this);
	}

	const_iterator end() const {
		return const_iterator(-1, *this);
	}

	void clear() {
		next_available = 0;
		active_end = 0;
		max_used_end = 0;
		n_actives = 0;
		next_key = 1001;

		for (auto& item : a) {
			item.t = T{};
			item.next_available = 0;
			item.active_id = 0;
			item.active = false;
		}
	}
};

};

