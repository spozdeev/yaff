#include <google/protobuf/util/message_differencer.h>
#include <yaff/yaff.h>

#include <fstream>
#include <iostream>

#include "schema.yaff.h"
#include "schema_light.yaff.h"

namespace {
interop::Everything Expected() {
    interop::Everything m;
    m.set_id(INT64_MIN);
    m.set_zero(0);
    m.set_text("hello");
    m.set_payload(std::string("\xff\0", 2));
    m.set_state(static_cast<interop::State>(-777));
    m.mutable_child()->set_note("");
    m.mutable_child()->set_counter(UINT64_MAX);
    for (int n : {-10, 0, 10}) m.add_numbers(n);
    m.add_words("");
    m.add_words("world");
    m.add_blobs(std::string("\xff\0", 2));
    m.add_states(interop::ACTIVE);
    m.add_states(static_cast<interop::State>(12345));
    m.add_children();
    *m.add_children() = m.child();
    auto* p = m.add_points();
    p->set_x(1);
    p->set_y(2);
    p = m.add_points();
    p->set_x(3);
    p->set_y(4);
    (*m.mutable_lookup())["z"] = m.child();
    (*m.mutable_lookup())["a"] = interop::Child{};
    (*m.mutable_labels())[-2] = "negative";
    (*m.mutable_labels())[0] = "";
    (*m.mutable_labels())[7] = "positive";
    m.set_zero_choice(0);
    m.mutable_defaults()->set_signed32(-123);
    m.mutable_defaults()->set_flag(true);
    m.mutable_defaults()->set_text("");
    m.mutable_defaults()->set_raw(std::string("\0\xff", 2));
    m.mutable_sparse()->set_first(-10);
    m.mutable_sparse()->set_tiny_last(true);
    m.mutable_sparse()->set_wide_first(UINT64_MAX);
    m.mutable_sparse()->set_last(-123);
    m.mutable_point()->set_x(3);
    m.mutable_point()->set_y(4);
    m.add_flags(true);
    m.add_flags(false);
    m.set_enabled(true);
    m.set_large(-123.5);
    m.set_small(1.25f);
    m.add_doubles(-0.0);
    m.add_doubles(1.5);
    m.mutable_timestamp()->set_seconds(-123);
    m.mutable_timestamp()->set_nanos(456);
    m.mutable_node()->set_value(1);
    m.mutable_node()->mutable_next()->set_value(2);
    return m;
}

void Check(const protoyaff::interop::Everything& m) {
    interop::Everything restored;
    m.ParseTo(restored);
    if (!google::protobuf::util::MessageDifferencer::Equals(restored, Expected())) {
        std::cerr << "Unexpected decoded message:\n" << restored.DebugString();
        throw std::runtime_error("generated interop value mismatch");
    }
    if (std::bit_cast<uint64_t>(m.doubles(0)) != 0x8000000000000000ULL) {
        throw std::runtime_error("negative zero was lost");
    }
}

void Write(const std::string& path, const interop::Everything& m, bool sparse) {
    yaff::Serializer serializer;
    if (sparse) serializer.EnforceDynamicAlternative(yaff::MessageLayout::MESSAGE_LAYOUT_SPARSE);
    auto buffer = yaff::Serialize<protoyaff::interop::Everything>(serializer, m);
    Check(yaff::ReadMessage<protoyaff::interop::Everything>(buffer.Data()));
    std::ofstream out(path, std::ios::binary);
    out.write(reinterpret_cast<const char*>(buffer.Data()), buffer.Size());
    if (!out) throw std::runtime_error("cannot write output");
}
}  // namespace

int main(int argc, char** argv) {
    try {
        if (argc < 3)
            throw std::runtime_error("usage: generated write-flat|write-sparse|check|transcode INPUT [OUTPUT]");
        const std::string mode(argv[1]);
        if (mode == "write-old" || mode == "write-new") {
            yaff::DetachedBuffer buffer;
            if (mode == "write-old") {
                interop::OldShape proto;
                proto.set_first(10);
                proto.set_retired(9);
                proto.set_tail(20);
                buffer = yaff::Serialize<protoyaff::interop::OldShape>(proto);
            } else {
                interop::NewShape proto;
                proto.set_first(10);
                proto.set_tail(20);
                proto.set_added("future");
                buffer = yaff::Serialize<protoyaff::interop::NewShape>(proto);
            }
            std::ofstream out(argv[2], std::ios::binary);
            out.write(reinterpret_cast<const char*>(buffer.Data()), buffer.Size());
            if (!out) throw std::runtime_error("cannot write schema evolution fixture");
        } else if (mode == "check-old" || mode == "check-new") {
            std::ifstream in(argv[2], std::ios::binary);
            if (!in) throw std::runtime_error("cannot read schema evolution fixture");
            std::string data{std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>()};
            if (mode == "check-old") {
                const auto& view = yaff::ReadMessage<protoyaff::interop::OldShape>(data.data());
                if (view.first() != 10 || view.retired() != 0 || view.tail() != 20)
                    throw std::runtime_error("old reader mismatch");
            } else {
                const auto& view = yaff::ReadMessage<protoyaff::interop::NewShape>(data.data());
                if (view.first() != 10 || view.tail() != 20 || !view.added().Empty())
                    throw std::runtime_error("new reader mismatch");
            }
        } else if (mode == "write-slice") {
            interop::Sliced proto;
            proto.set_id(77);
            proto.set_extra("excluded");
            auto buffer = yaff::Serialize<light::interop::Sliced>(proto);
            std::ofstream out(argv[2], std::ios::binary);
            out.write(reinterpret_cast<const char*>(buffer.Data()), buffer.Size());
            if (!out) throw std::runtime_error("cannot write slice");
        } else if (mode == "check-slice") {
            std::ifstream in(argv[2], std::ios::binary);
            if (!in) throw std::runtime_error("cannot read slice");
            std::string data{std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>()};
            const auto& view = yaff::ReadMessage<light::interop::Sliced>(data.data());
            if (view.id() != 77) throw std::runtime_error("slice ID mismatch");
        } else if (mode == "write-flat" || mode == "write-sparse") {
            Write(argv[2], Expected(), mode == "write-sparse");
        } else {
            std::ifstream in(argv[2], std::ios::binary);
            if (!in) throw std::runtime_error("cannot read input");
            std::string data{std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>()};
            const auto& m = yaff::ReadMessage<protoyaff::interop::Everything>(data.data());
            Check(m);
            if (mode == "transcode") {
                if (argc != 4) throw std::runtime_error("transcode needs output path");
                interop::Everything proto;
                m.ParseTo(proto);
                Write(argv[3], proto, true);
            } else if (mode != "check")
                throw std::runtime_error("unknown action");
        }
        std::cout << "Generated C++ interop: passed\n";
    } catch (const std::exception& e) {
        std::cerr << e.what() << '\n';
        return 1;
    }
}
