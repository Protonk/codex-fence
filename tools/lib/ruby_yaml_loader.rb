require 'yaml'

module CodexFence
  module RubyYamlLoader
    module_function

    def safe_load_file(path)
      safe_load_string(File.read(path))
    end

    def safe_load_string(contents)
      if keyword_safe_load_supported?
        YAML.safe_load(
          contents,
          permitted_classes: [],
          permitted_symbols: [],
          aliases: true
        )
      else
        YAML.safe_load(contents, [], [], true)
      end
    end

    def keyword_safe_load_supported?
      major, minor = RUBY_VERSION.split('.').map(&:to_i)
      major > 3 || (major == 3 && minor >= 1)
    end
  end
end
