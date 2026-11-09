#include <compilation/proto_front.h>
#include <compilation/rust_gen.h>
#include <google/protobuf/compiler/code_generator.h>
#include <google/protobuf/compiler/plugin.h>
#include <google/protobuf/io/printer.h>

#include <filesystem>
#include <sstream>

namespace {

class RustPlugin final : public google::protobuf::compiler::CodeGenerator {
public:
    bool Generate(const google::protobuf::FileDescriptor* file, const std::string& parameter,
                  google::protobuf::compiler::GeneratorContext* context, std::string* error) const override {
        return GenerateAll({file}, parameter, context, error);
    }

    bool GenerateAll(const std::vector<const google::protobuf::FileDescriptor*>& files, const std::string& parameter,
                     google::protobuf::compiler::GeneratorContext* context, std::string* error) const override {
        try {
            yaff::compilation::ProtobufReflectionFrontEndOptions opts;
            opts.SkipEmptyMessages = false;
            // An empty whitelist includes referenced imports in the same module
            // tree, avoiding duplicate Rust modules for files in one package.
            std::string tag;
            std::string output = "yaff.rs";
            bool trusted = false;
            std::vector<std::pair<std::string, std::string>> parameters;
            google::protobuf::compiler::ParseGeneratorParameter(parameter, &parameters);
            for (const auto& [key, value] : parameters) {
                if (key == "namespace")
                    opts.RootNamespace = value;
                else if (key == "tag")
                    tag = value;
                else if (key == "output")
                    output = value;
                else if (key == "trusted" && (value.empty() || value == "true" || value == "false"))
                    trusted = value != "false";
                else
                    throw std::runtime_error("unknown YaFF Rust option: " + key);
            }
            const std::filesystem::path path(output);
            if (path.empty() || path.is_absolute()) throw std::runtime_error("output must be a relative file name");
            for (const auto& part : path)
                if (part == "..") throw std::runtime_error("output cannot contain '..'");
            std::unique_ptr<yaff::compilation::ProtobufReflectionTraits> traits;
            if (tag.empty())
                traits = std::make_unique<yaff::compilation::DefaultProtobufReflectionTraits>();
            else
                traits = std::make_unique<yaff::compilation::TaggedProtobufReflectionTraits>(tag);
            auto front =
                std::make_unique<yaff::compilation::ProtobufReflectionFrontEnd>(files, std::move(traits), opts);
            std::ostringstream generated;
            std::vector<std::unique_ptr<yaff::compilation::AbstractGenerator>> generators;
            generators.emplace_back(std::make_unique<yaff::compilation::RustGenerator>(generated, trusted));
            yaff::compilation::Compile(std::move(front), std::move(generators), nullptr, nullptr);
            std::unique_ptr<google::protobuf::io::ZeroCopyOutputStream> stream(context->Open(output));
            google::protobuf::io::Printer printer(stream.get());
            const auto content = generated.str();
            printer.WriteRaw(content.data(), content.size());
            return true;
        } catch (const std::exception& e) {
            *error = e.what();
            return false;
        }
    }

    uint64_t GetSupportedFeatures() const override {
        return FEATURE_PROTO3_OPTIONAL;
    }
};
}  // namespace

int main(int argc, char** argv) {
    RustPlugin generator;
    return google::protobuf::compiler::PluginMain(argc, argv, &generator);
}
