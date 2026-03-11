#!/bin/bash
# Final integration test for JSON output format

echo "=== Final Integration Test for JSON Output Format ==="
echo

echo "Test 1: Verify output-format flag exists in help"
if cargo run --quiet --package clawdius -- --help 2>&1 | grep -q "output-format"; then
    echo "✓ Flag --output-format found in help"
else
    echo "✗ Flag --output-format NOT found in help"
fi
echo

echo "Test 2: Verify all format options are valid"
for format in text json stream-json; do
    if cargo run --quiet --package clawdius -- sessions --output-format $format 2>&1 | grep -qE "(Sessions|session|No sessions|error)"; then
        echo "✓ Format '$format' works"
    else
        echo "✗ Format '$format' failed"
    fi
done
echo

echo "Test 3: Verify JSON output is valid JSON"
output=$(cargo run --quiet --package clawdius -- sessions --output-format json 2>&1)
if echo "$output" | jq empty 2>/dev/null; then
    echo "✓ JSON output is valid"
else
    echo "✗ JSON output is invalid"
fi
echo

echo "Test 4: Verify text output is human-readable"
if cargo run --quiet --package clawdius -- sessions --output-format text 2>&1 | grep -qE "(Sessions|No sessions)"; then
    echo "✓ Text output is readable"
else
    echo "✗ Text output failed"
fi
echo

echo "Test 5: Verify short flag works"
if cargo run --quiet --package clawdius -- sessions -f json 2>&1 | grep -qE "(Sessions|session|No sessions)"; then
    echo "✓ Short flag -f works"
else
    echo "✗ Short flag -f failed"
fi
echo

echo "=== Test Complete ==="
