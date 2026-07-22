Feature: Tool call lifecycle
  Scenario: Happy path lists the current directory through a tool
    # This executable slice asserts the public exec stream and mocked LLM border.
    # The real Unix PTY screen slice in qa_pty.rs asserts the matching Work,
    # statusline, idle BlueWhale, live tool-card, and settled-transcript border.
    Given an offline CodeWhale workspace containing:
      | path      | kind   |
      | README.md | file   |
      | notes.txt | file   |
      | src       | folder |
    And the mocked LLM will request the "list_dir" tool with:
      | path |
      | .    |
    And the mocked LLM will answer after the tool result:
      | content                                                    |
      | The directory contains README.md, notes.txt, and src/.      |
    When the user asks "list the current directory"
    Then CodeWhale should send the user request to the mocked LLM
    And the public tool lifecycle should show a running tool:
      | status  | marker | tool     | input |
      | running | [~]    | list_dir | .     |
    And the public tool result should return directory entries:
      | entry     | kind   |
      | README.md | file   |
      | notes.txt | file   |
      | src       | folder |
    And CodeWhale should send the tool result back to the mocked LLM
    And the public tool lifecycle should show a completed tool:
      | status    | marker | tool     | input |
      | completed | ✓      | list_dir | .     |
    And the public output should include "The directory contains README.md, notes.txt, and src/."

  Scenario: Unknown tool returns an error result
    Given an offline CodeWhale workspace containing:
      | path      | kind |
      | README.md | file |
    And the mocked LLM will request the "missing_tool" tool with:
      | path |
      | .    |
    And the mocked LLM will answer after the tool result:
      | content                                      |
      | I could not run the requested missing tool. |
    When the user asks "try a missing tool"
    Then CodeWhale should send the user request to the mocked LLM
    And the public tool lifecycle should show a running tool:
      | status  | marker | tool         | input |
      | running | [~]    | missing_tool | .     |
    And the public tool result should report an error for "missing_tool"
    And CodeWhale should send the tool error back to the mocked LLM
    And the public tool lifecycle should show a failed tool:
      | status | marker | tool         | input |
      | error  | [!]    | missing_tool | .     |
    And the public output should include "I could not run the requested missing tool."

  Scenario: Malformed tool arguments return an error result
    Given an offline CodeWhale workspace containing:
      | path      | kind |
      | README.md | file |
    And the mocked LLM will request the "list_dir" tool with malformed arguments "{not-json"
    And the mocked LLM will answer after the tool result:
      | content                                 |
      | I could not parse the tool arguments. |
    When the user asks "try malformed tool arguments"
    Then CodeWhale should send the user request to the mocked LLM
    And the public tool lifecycle should show a running tool with raw input for "list_dir"
    And the public tool result should report malformed arguments for "list_dir"
    And CodeWhale should send the malformed argument error back to the mocked LLM
    And the public tool lifecycle should show a failed tool with raw input for "list_dir"
    And the public output should include "I could not parse the tool arguments."

  Scenario: A real tool error is returned to the follow-up request
    Given an offline CodeWhale workspace containing:
      | path      | kind |
      | README.md | file |
    And the mocked LLM will request the "read_file" tool with:
      | path        |
      | missing.txt |
    And the mocked LLM will answer after the tool result:
      | content                                                  |
      | I could not read missing.txt because the file is absent. |
    When the user asks "read the missing file"
    Then CodeWhale should send the user request to the mocked LLM
    And the public tool lifecycle should show a running tool:
      | status  | marker | tool      | input       |
      | running | [~]    | read_file | missing.txt |
    And the public tool result should report a real error for "read_file" containing "missing.txt"
    And CodeWhale should send the real tool error back to the mocked LLM
    And the public tool lifecycle should show a failed tool:
      | status | marker | tool      | input       |
      | error  | [!]    | read_file | missing.txt |
    And the public output should include "I could not read missing.txt because the file is absent."

  Scenario: An empty tool result is returned to the follow-up request
    Given an offline CodeWhale workspace containing:
      | path  | kind   |
      | empty | folder |
    And the mocked LLM will request the "list_dir" tool with:
      | path  |
      | empty |
    And the mocked LLM will answer after the tool result:
      | content                           |
      | The directory is currently empty. |
    When the user asks "list the empty directory"
    Then CodeWhale should send the user request to the mocked LLM
    And the public tool lifecycle should show a running tool:
      | status  | marker | tool     | input |
      | running | [~]    | list_dir | empty |
    And the public tool result should be an empty list
    And CodeWhale should send the empty tool result back to the mocked LLM
    And the public tool lifecycle should show a completed tool:
      | status    | marker | tool     | input |
      | completed | ✓      | list_dir | empty |
    And the public output should include "The directory is currently empty."

  Scenario: A follow-up answer missing the expected summary is detected
    Given an offline CodeWhale workspace containing:
      | path      | kind |
      | README.md | file |
    And the mocked LLM will request the "list_dir" tool with:
      | path |
      | .    |
    And the mocked LLM will answer after the tool result:
      | content                    |
      | I inspected the workspace. |
    When the user asks "summarize the current directory"
    Then CodeWhale should send the user request to the mocked LLM
    And the public tool lifecycle should show a running tool:
      | status  | marker | tool     | input |
      | running | [~]    | list_dir | .     |
    And the public tool result should return directory entries:
      | entry     | kind |
      | README.md | file |
    And CodeWhale should send the tool result back to the mocked LLM
    And the public tool lifecycle should show a completed tool:
      | status    | marker | tool     | input |
      | completed | ✓      | list_dir | .     |
    And the public output should include "I inspected the workspace."
    But acceptance should report the missing expected summary "The directory contains README.md."
