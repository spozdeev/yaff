// A dependency-free C++ oracle for the language-independent wire corpus.
#include <yaff/yaff.h>

#include <array>
#include <bit>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

namespace {

struct SimpleMeta {
    static constexpr size_t LIMIT = 17;
    static constexpr std::array<yaff::FieldOffset, 5> FLAT_OFFSETS = {0, 1, 5, 13, 17};
    static constexpr std::array<yaff::FieldId, 0> DELETED_IDS = {};
    static constexpr std::array<bool, 4> STATIC_FLAGS = {true, true, true, true};
};
struct RootMeta {
    static constexpr std::array<yaff::FieldOffset, 7> FLAT_OFFSETS = {0, 4, 8, 12, 16, 20, 24};
    static constexpr std::array<yaff::FieldId, 0> DELETED_IDS = {};
    static constexpr std::array<bool, 6> STATIC_FLAGS = {true, true, true, true, true, true};
};
struct PairMeta {
    static constexpr size_t LIMIT = 12;
    static constexpr std::array<yaff::FieldOffset, 3> FLAT_OFFSETS = {0, 4, 12};
};
struct OldMeta {
    static constexpr std::array<yaff::FieldOffset, 6> FLAT_OFFSETS = {0, 8, 12, 20, 21, 29};
    static constexpr std::array<yaff::FieldId, 0> DELETED_IDS = {};
    static constexpr std::array<bool, 5> STATIC_FLAGS = {true, true, true, true, true};
};
struct NewMeta {
    static constexpr std::array<yaff::FieldOffset, 7> FLAT_OFFSETS = {0, 8, 8, 8, 9, 17, 21};
    static constexpr std::array<yaff::FieldId, 2> DELETED_IDS = {2, 3};
    static constexpr std::array<bool, 6> STATIC_FLAGS = {true, true, true, false, false, false};
};

void Require(bool condition) {
    if (!condition) throw std::runtime_error("interop oracle check failed");
}

void Save(const std::filesystem::path& path, const yaff::Serializer& s) {
    std::ofstream out(path, std::ios::binary);
    out.write(reinterpret_cast<const char*>(s.Data()), s.Size());
    Require(bool(out));
}

std::vector<char> Load(const std::filesystem::path& path) {
    std::ifstream in(path, std::ios::binary);
    Require(bool(in));
    return {std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>()};
}

template <class M>
void CheckSimple(const void* data, bool presence) {
    const auto& m = yaff::ReadMessage<M>(data);
    Require(m.template ReadValue<bool>(1, false));
    Require(m.template ReadValue<uint32_t>(2, 175) == 175);
    Require(m.template ReadPresence<uint32_t>(2) == presence);
    Require(m.template ReadValue<double>(3, 1.25) == -12.5);
    const auto* str = m.template ReadLayout<yaff::String>(4);
    Require(str && str->AsStringView() == std::string_view("A\0B", 3));
    Require(str->Data()[3] == 0);
}

void Simple(const std::filesystem::path& dir, bool write) {
    for (const auto layout : {"fixed", "flat", "sparse"}) {
        for (bool implicit : {false, true}) {
            for (bool sized : {false, true}) {
                if (std::string_view(layout) != "flat" && sized) continue;
                if (std::string_view(layout) == "fixed" && implicit) continue;
                const auto path = dir / (std::string(layout) + (implicit ? "_implicit" : "_explicit") +
                                         (sized ? "_sized.bin" : ".bin"));
                if (write) {
                    yaff::Serializer s;
                    auto str = s.SerializeString(std::string_view("A\0B", 3));
                    if (std::string_view(layout) == "fixed")
                        s.StartFixedMessage<SimpleMeta>();
                    else if (std::string_view(layout) == "flat")
                        s.StartFlatMessage<SimpleMeta>(implicit, sized);
                    else
                        s.StartSparseMessage(implicit);
                    s.AddField(4, str);
                    s.AddField<double>(3, -12.5, 1.25);
                    s.AddField<uint32_t>(2, 175, 175);
                    s.AddField<bool>(1, true, false);
                    auto offset = std::string_view(layout) == "fixed"  ? s.FinishFixedMessage()
                                  : std::string_view(layout) == "flat" ? s.FinishFlatMessage()
                                                                       : s.FinishSparseMessage();
                    s.Finish(yaff::InternalOffset<>(offset));
                    Save(path, s);
                }
                auto data = Load(path);
                if (std::string_view(layout) == "fixed")
                    CheckSimple<yaff::FixedMessage<SimpleMeta>>(data.data(), false);
                else
                    CheckSimple<yaff::DynamicMessage<SimpleMeta>>(data.data(), !implicit);
            }
        }
    }
}

void Collections(const std::filesystem::path& dir, bool write) {
    const auto path = dir / "collections.bin";
    if (write) {
        yaff::Serializer s;
        auto nums = s.SerializeArray(std::vector<int64_t>{INT64_MIN, -1, 0, INT64_MAX});
        auto flags = s.SerializeArray(std::vector<bool>{true, false, true});
        auto empty = s.SerializeString("");
        auto raw = s.SerializeString(std::string_view("\xff\0z", 3));
        auto strings = s.SerializeArray(std::vector{empty, raw, empty});
        s.StartSparseMessage();
        s.AddField<uint32_t>(1, 10, 0);
        auto first = yaff::InternalOffset<yaff::SparseMessage>(s.FinishSparseMessage());
        s.StartSparseMessage();
        s.AddField<uint32_t>(1, 20, 0);
        auto second = yaff::InternalOffset<yaff::SparseMessage>(s.FinishSparseMessage());
        auto messages = s.SerializeArray(std::vector{first, second});
        auto pairs = s.SerializeArray<yaff::InlineOffset<yaff::FixedMessage<PairMeta>>>([&](size_t i) {
            s.StartFixedMessage<PairMeta>();
            s.AddField<uint64_t>(2, 200 - i * 100, 0);
            s.AddField<int32_t>(1, 2 - static_cast<int32_t>(i), 0);
            return std::make_pair(yaff::InlineOffset<yaff::FixedMessage<PairMeta>>(s.FinishFixedMessage()), i == 0);
        });
        s.StartFlatMessage<RootMeta>(false, true);
        s.AddField(6, empty);
        s.AddField(5, pairs);
        s.AddField(4, messages);
        s.AddField(3, strings);
        s.AddField(2, flags);
        s.AddField(1, nums);
        s.Finish(yaff::InternalOffset<>(s.FinishFlatMessage()));
        Save(path, s);
    }
    auto data = Load(path);
    const auto& m = yaff::ReadMessage<yaff::DynamicMessage<RootMeta>>(data.data());
    auto nums = m.ReadLayout<yaff::Array<int64_t>>(1);
    Require(nums->Size() == 4 && nums->Get(0) == INT64_MIN && nums->Get(3) == INT64_MAX);
    auto flags = m.ReadLayout<yaff::Array<bool>>(2);
    Require(flags->Size() == 3 && flags->Get(0) && !flags->Get(1) && flags->Get(2));
    auto strings = m.ReadLayout<yaff::Array<yaff::InternalOffset<yaff::String>>>(3);
    Require(strings->Size() == 3 && strings->Get(0).Empty() && strings->Get(2).Empty());
    Require(strings->Get(1).AsStringView() == std::string_view("\xff\0z", 3));
    auto messages = m.ReadLayout<yaff::Array<yaff::InternalOffset<yaff::SparseMessage>>>(4);
    Require(messages->Size() == 2 && messages->Get(0).ReadValue<uint32_t>(1, 0) == 10);
    Require(messages->Get(1).ReadValue<uint32_t>(1, 0) == 20);
    auto pairs = m.ReadLayout<yaff::Array<yaff::InlineOffset<yaff::FixedMessage<PairMeta>>>>(5);
    Require(pairs->Size() == 2 && pairs->Get(0).ReadValue<int32_t>(1, 0) == 1);
    Require(pairs->Get(1).ReadValue<uint64_t>(2, 0) == 200);
    Require(m.ReadPresence<yaff::String>(6) && m.ReadLayout<yaff::String>(6)->Empty());
}

void Evolution(const std::filesystem::path& dir, bool write) {
    for (bool explicitDefault : {false, true}) {
        const auto path = dir / (explicitDefault ? "evolution_explicit.bin" : "evolution_implicit.bin");
        if (write) {
            yaff::Serializer s;
            s.StartFlatMessage<OldMeta>(false, true);
            s.AddField<uint64_t>(5, explicitDefault ? 10 : 11, 10);
            s.AddField<bool>(4, true, false);
            s.AddField<uint64_t>(3, 15, 10);
            s.AddField<uint32_t>(2, 3, 0);
            s.AddField<uint64_t>(1, 20, 0);
            s.Finish(yaff::InternalOffset<>(s.FinishFlatMessage()));
            Save(path, s);
        }
        auto data = Load(path);
        const auto& m = yaff::ReadMessage<yaff::DynamicMessage<NewMeta>>(data.data());
        Require(m.ReadValue<uint64_t>(1, 0) == 20);
        Require(m.ReadValue<bool>(4, false));
        Require(m.ReadValue<uint64_t>(5, 10) == (explicitDefault ? 10 : 11));
        Require(m.ReadPresence<uint64_t>(5));
        Require(!m.ReadPresence<uint32_t>(6) && m.ReadValue<uint32_t>(6, 1337) == 1337);
    }
}

void Boundaries(const std::filesystem::path& dir, bool write) {
    const auto path = dir / "sparse_boundaries.bin";
    if (write) {
        yaff::Serializer s;
        s.StartSparseMessage();
        s.AddField<int32_t>(8190, -123, 0);
        s.AddField<uint64_t>(32, UINT64_MAX, 0);
        s.AddField<bool>(31, true, false);
        s.AddField<int32_t>(1, -777, 0);  // Also represents an unknown enum value.
        s.Finish(yaff::InternalOffset<>(s.FinishSparseMessage()));
        Save(path, s);
    }
    auto data = Load(path);
    const auto& m = yaff::ReadMessage<yaff::SparseMessage>(data.data());
    Require(m.ReadValue<int32_t>(8190, 0) == -123);
    Require(m.ReadValue<uint64_t>(32, 0) == UINT64_MAX);
    Require(m.ReadValue<bool>(31, false));
    Require(m.ReadValue<int32_t>(1, 0) == -777);
    Require(!m.ReadPresence<uint32_t>(30));
    for (bool flat : {false, true}) {
        const auto emptyPath = dir / (flat ? "empty_flat.bin" : "empty_sparse.bin");
        if (write) {
            yaff::Serializer s;
            if (flat)
                s.StartFlatMessage<SimpleMeta>();
            else
                s.StartSparseMessage();
            s.Finish(yaff::InternalOffset<>(flat ? s.FinishFlatMessage() : s.FinishSparseMessage()));
            Save(emptyPath, s);
        }
        auto empty = Load(emptyPath);
        const auto& m = yaff::ReadMessage<yaff::DynamicMessage<SimpleMeta>>(empty.data());
        Require(!m.ReadPresence<bool>(1) && m.ReadValue<uint32_t>(2, 175) == 175);
    }
}
void RootValues(const std::filesystem::path& dir, bool write) {
    const auto stringPath = dir / "root_bytes.bin";
    const auto arrayPath = dir / "root_array.bin";
    if (write) {
        yaff::Serializer str;
        str.Finish(str.SerializeString(std::string_view("\xff\0z", 3)));
        Save(stringPath, str);
        yaff::Serializer array;
        array.Finish(array.SerializeArray(std::vector<int64_t>{INT64_MIN, 0, INT64_MAX}));
        Save(arrayPath, array);
    }
    auto string = Load(stringPath);
    Require(yaff::ReadMessage<yaff::String>(string.data()).AsStringView() == std::string_view("\xff\0z", 3));
    auto array = Load(arrayPath);
    const auto& view = yaff::ReadMessage<yaff::Array<int64_t>>(array.data());
    Require(view.Size() == 3 && view.Get(0) == INT64_MIN && view.Get(1) == 0 && view.Get(2) == INT64_MAX);
}
}  // namespace

int main(int argc, char** argv) {
    try {
        if (argc != 3 || (std::string_view(argv[1]) != "write" && std::string_view(argv[1]) != "check")) {
            std::cerr << "usage: oracle write|check DIRECTORY\n";
            return 2;
        }
        if (std::endian::native != std::endian::little) throw std::runtime_error("little-endian host required");
        const bool write = std::string_view(argv[1]) == "write";
        const std::filesystem::path dir(argv[2]);
        if (write) std::filesystem::create_directories(dir);
        Simple(dir, write);
        Collections(dir, write);
        Evolution(dir, write);
        Boundaries(dir, write);
        RootValues(dir, write);
        std::cout << "C++ wire corpus: all checks passed\n";
    } catch (const std::exception& e) {
        std::cerr << e.what() << '\n';
        return 1;
    }
}
