// Runtime coverage is split into a parser/loader smoke test and focused
// assertions. Their shared `.tn` fixture is grouped by language feature.
mod fixture;
mod parses_and_loads;
mod pattern_matching;
