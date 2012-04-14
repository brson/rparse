#[doc = "Generic parse functions."];		// TODO: expand on this a bit (eg mention top level functions)

import io;
import io::writer_util;
import result::*;
import types::*;

const EOT: char = '\u0003';

// ---- Helper Functions ------------------------------------------------------
// TODO: don't export these.

// This is designed to speed up parsing because the parsers don't have 
// to repeatedly verify that index is in range.
//
// Of course converting a string to a vector is not especially efficient, but
// it will be faster than handling utf-8 (unless we can guarantee that it
// is all 7-bit ASCII of course).
#[doc = "Like str::chars except that END OF TEXT (\u0003) is appended."]
fn chars_with_eot(s: str) -> [char]
{
    let mut buf = [], i = 0u;
    let len = str::len(s);
    while i < len
    {
        let {ch, next} = str::char_range_at(s, i);
        assert next > i;
        buf += [ch];
        i = next;
    }
    buf += [EOT];
    ret buf;
}

// Note that, unlike the functions in the char module, these are 7-bit ASCII functions.
fn is_alpha(ch: char) -> bool
{
	ret (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z');
}

fn is_digit(ch: char) -> bool
{
	ret ch >= '0' && ch <= '9';
}

fn is_alphanum(ch: char) -> bool
{
	ret is_alpha(ch) || is_digit(ch);
}

fn is_print(ch: char) -> bool
{
	ret ch >= ' ' && ch <= '~';
}

fn repeat_char(ch: char, count: uint) -> str
{
	let mut value = "";
	str::reserve(value, count);
	uint::range(0u, count) {|_i| str::push_char(value, ch);}
	ret value;
}

// Note that we don't want to escape control characters here because we need
// one code point to map to one printed character (so our plog arrows point to
// the right character).
fn munge_chars(chars: [char]) -> str
{
	// TODO: I'd like to use bullet here, but while io::println handles it correctly
	// the logging subsystem does not. See issue 2154.
	//let bullet = '\u2022';
	let bullet = '.';
	
	let mut value = "";
	str::reserve(value, vec::len(chars));
	vec::iter(chars) {|ch| str::push_char(value, if is_print(ch) {ch} else {bullet});}
	ret value;
}

fn eot<T: copy>(answer: state<T>) -> status<T>
{
	if answer.text[answer.index] == '\u0003'
	{
		ret result::ok(answer);
	}
	else
	{
		let last = uint::min(answer.index + 16u, vec::len(answer.text) - 1u);
		let trailer = str::from_chars(vec::slice(answer.text, answer.index, last));
		ret result::err({output: answer, maxIndex: answer.index, mesg: #fmt["expected EOT but found '%s'", trailer]});
	}
}

fn binary_op_arms<T: copy>(input: state<T>, arms: [(parser<T>, parser<T>, fn@ (T, T) -> result::result<T, str>)]) -> status<T>
{
	let mut i = 0u;
	let mut maxIndex = input.index;
	while i < vec::len(arms)
	{
		let (op, rhs, eval) = arms[i];
		alt op(input)
		{
			result::ok(opOut)
			{
				alt rhs(opOut)
				{
					result::ok(rhsOut)
					{
						alt eval(input.value, rhsOut.value)
						{
							result::ok(value)
							{
								ret result::ok({value: value with rhsOut});
							}
							result::err(mesg)
							{
								ret result::err({output: input, maxIndex: maxIndex, mesg: mesg});
							}
						}
					}
					result::err(error)
					{
						maxIndex = uint::max(maxIndex, error.maxIndex);
					}
				}
			}
			result::err(error)
			{
				maxIndex = uint::max(maxIndex, error.maxIndex);
			}
		}
		i += 1u;
	}
	ret result::err({output: input, maxIndex: maxIndex, mesg: ""});		// this is not really an error: it just signals binary_op that it is done
}

// ---- Parse Functions -------------------------------------------------------
#[doc = "Used to log the results of a parse function (at both info and debug levels).

Typical usage is to call this function with whatever the parse function wants to return:

   ret plog(\"my_parser\", input, output);"]
fn plog<T: copy>(fun: str, input: state<T>, output: status<T>) -> status<T>
{
	alt output
	{
		result::ok(answer)
		// Note that we make multiple calls to munge_chars which is fairly slow, but
		// we only do that when actually logging: when info or debug logging is off
		// the munge_chars calls aren't evaluated.
		{
			assert answer.index >= input.index;				// can't go backwards on success (but no progress is fine, eg e*)
			if answer.index > input.index
			{
				#info("%s", munge_chars(input.text));
				#info("%s^ %s parsed '%s'", repeat_char(' ', answer.index), fun, str::slice(munge_chars(input.text), input.index, answer.index));
			}
			else
			{
				#debug("%s", munge_chars(input.text));
				#debug("%s^ %s passed", repeat_char(' ', answer.index), fun);
			}
		}
		result::err(error)
		{
			assert error.output.index == input.index;		// on errors the next parser must begin at the start
			#debug("%s", munge_chars(input.text));
			#debug("%s^ %s failed", repeat_char('-', input.index), fun);
		}
	}
	ret output;
}

#[doc = "A parser that always fails."]
fn fails<T: copy>(input: state<T>) -> status<T>
{
	ret plog("fails", input, result::err({output: input, maxIndex: input.index, mesg: "forced failure"}));
}

// This (and some of the other functions) handle repetition themselves
// for efficiency. It also has a very short name because it is a very commonly
// used function.
#[doc = "space_zero_or_more := (' ' | '\t' | '\r' | '\n')*"]
fn space_zero_or_more<T: copy>(input: state<T>) -> status<T>
{
	let mut i = input.index;
	let mut line = input.line;
	while true
	{
		if input.text[i] == '\r' && input.text[i+1u] == '\n'
		{
			line += 1;
			i += 1u;
		}
		else if input.text[i] == '\n'
		{
			line += 1;
		}
		else if input.text[i] == '\r'
		{
			line += 1;
		}
		else if input.text[i] != ' ' && input.text[i] != '\t'
		{
			break;
		}
		i += 1u;
	}
	
	ret plog("s", input, result::ok({index: i, line: line with input}));
}

#[doc = "space_zero_or_more := (' ' | '\t' | '\r' | '\n')+"]
fn space_one_or_more<T: copy>(input: state<T>) -> status<T>
{
	let result = space_zero_or_more(input);
	let state = result::get(result);
	
	if state.index > input.index
	{
		ret plog("spaces", input, result);
	}
	else
	{
		ret plog("spaces", input, result::err({output: input, maxIndex: input.index, mesg: "expected whitespace"}));
	}
}

#[doc = "literal := <literal> space"]
fn literal<T: copy>(input: state<T>, literal: str, space: parser<T>) -> status<T>
{
	let mut i = 0u;
	let mut j = input.index;
	while i < str::len(literal)
	{
		let {ch, next} = str::char_range_at(literal, i);
		assert next > i;
		if ch == input.text[j]
		{
			i = next;
			j += 1u;
		}
		else
		{
			ret plog(#fmt["literal '%s'", literal], input, result::err({output: input, maxIndex: j, mesg: #fmt["expected '%s'", literal]}));
		}
	}
	
	ret plog("literal", input, space({index: j with input}));
}

#[doc = "integer := [+-]? [0-9]+ space"]
fn integer<T: copy>(input: state<T>, space: parser<T>, eval: fn@ (int) -> T) -> status<T>
{
	let mut start = input.index;
	if input.text[start] == '+' || input.text[start] == '-'
	{
		start += 1u;
	}
	
	let mut i = start;
	while is_digit(input.text[i])
	{
		i += 1u;
	}
	
	if i == start
	{
		ret plog("integer", input, result::err({output: input, maxIndex: start, mesg: "expected an integer"}));
	}
	
	alt space({index: i with input})		// TODO: not sure if we can simplify this with chain (type inference has problems figuring out the type of the closure)
	{
		result::ok(answer)
		{
			let text = str::from_chars(vec::slice(input.text, start, i));
			let mut value = option::get(int::from_str(text));
			if input.text[input.index] == '-'
			{
				value = -value;
			}
			ret plog("integer", input, result::ok({value: eval(value) with answer}));
		}
		result::err(error)
		{
			ret plog("integer", input, result::err(error));
		}
	}
}

#[doc = " identifier := [a-zA-Z] [a-zA-Z0-9_]* space"]
fn identifier<T: copy>(input: state<T>, space: parser<T>, eval: fn@ (str) -> T) -> status<T>
{
	if !is_alpha(input.text[input.index])
	{
		ret plog("identifier", input, result::err({output: input, maxIndex: input.index, mesg: "expected an element name"}));
	}
	
	let start = input.index;
	let mut i = start;
	while is_alphanum(input.text[i]) || input.text[i] == '_'
	{
		i += 1u;
	}
	
	let s = str::from_chars(vec::slice(input.text, start, i));
	let answer = get(space({index: i with input}));
	ret plog("identifier", input, result::ok({value: eval(s) with answer}));
}

#[doc = "terms := lhs (op rhs)*

Where arms is a vector of (op, rhs, eval)."]
fn binary_op<T: copy>(input: state<T>, lhs: parser<T>, arms: [(parser<T>, parser<T>, fn@ (T, T) -> result::result<T, str>)]) -> status<T>
{
	let result = lhs(input);
	alt result
	{
		result::ok(answer)
		{
			let mut out = result;
			while true
			{
				let out2 = binary_op_arms(get(out), arms);
				alt out2
				{
					result::ok(rhs)
					{
						assert rhs.index > get(out).index;	// arm must fail or make progress
						out = out2;
					}
					result::err(error)
					{
						if str::is_empty(error.mesg)
						{
							ret plog("binary_op", input, out);	// we expect arms to fail eventually
						}
						else
						{
							ret plog("binary_op", input, result::err({output: input with error}));
						}
					}
				}
			}
			ret plog("binary_op", input, out);			// keep the compiler happy
		}
		result::err(error)
		{
			ret plog("binary_op", input, result::err(error));
		}
	}
}

#[doc = "zero_or_more := e*

Eval is called with the value of each parsed e."]
fn repeat_zero_or_more<T: copy>(input: state<T>, parser: parser<T>, eval: fn@ ([T]) -> result::result<T, str>) -> status<T>
{
	let mut out = input;
	let mut values = [];
	loop
	{
		alt parser(out)
		{
			result::ok(answer)
			{
				assert answer.index > out.index;		// must make progress to guarantee loop termination
				vec::push(values, answer.value);
				out = answer;
			}
			result::err(error)
			{
				alt eval(values)
				{
					result::ok(value)
					{
						ret plog("repeat_zero_or_more", input, result::ok({value: value with out}));
					}
					result::err(mesg)
					{
						ret plog("repeat_zero_or_more", input, result::err({output: input, maxIndex: out.index, mesg: mesg}));
					}
				}
			}
		}
	}
}

#[doc = "one_or_more := e+

Eval is called with the value of each parsed e."]
fn repeat_one_or_more<T: copy>(input: state<T>, parser: parser<T>, eval: fn@ ([T]) -> result::result<T, str>, errMesg: str) -> status<T>
{
	let out = get(repeat_zero_or_more(input, parser, eval));
	if out.index > input.index
	{
		ret plog("repeat_one_or_more", input, result::ok(out));
	}
	else
	{
		ret plog("repeat_one_or_more", input, result::err({output: input, maxIndex: out.index, mesg: errMesg}));
	}
}

#[doc = "optional := e?

Eval is called with either [] or the value of e."]
fn optional<T: copy>(input: state<T>, parser: parser<T>, eval: fn@ ([T]) -> result::result<T, str>) -> status<T>
{
	ret repeat_zero_or_more(input, parser, eval);
}

#[doc = "alternative := e1 | e2 | e3…"]
fn alternative<T: copy>(input: state<T>, parsers: [parser<T>]) -> status<T>
{
	let mut i = 0u;
	let mut maxIndex = input.index;
	let mut errMesg = "";
	let mut messages: [str] = [];
	
	while i < vec::len(parsers)
	{
		let result = parsers[i](input);
		alt result
		{
			result::ok(answer)
			{
				ret plog("alternative", input, result);
			}
			result::err(error)
			{
				if error.maxIndex > maxIndex
				{
					maxIndex = error.maxIndex;
					errMesg = error.mesg;
				}
				else if !vec::contains(messages, error.mesg)
				{
					vec::push(messages, error.mesg);
				}
			}
		}
		i += 1u;
	}
	
	// If the alternatives were able to process anything then we'll use the error message of the one that processed the most.
	// Otherwise none of them were able to process anything so we'll print what each expected.
	if str::is_empty(errMesg)
	{
		ret plog("alternative", input, result::err({output: input, maxIndex: maxIndex, mesg: str::connect(messages, " or ")}));
	}
	else
	{
		ret plog("alternative", input, result::err({output: input, maxIndex: maxIndex, mesg: errMesg}));
	}
}

#[doc = "sequence := e1 e2 e3…

Eval will be called with the values from each e."]
fn sequence<T: copy>(input: state<T>, parsers: [parser<T>], eval: fn@ ([T]) -> result::result<T, str>) -> status<T>
{
	let mut values = [];
	vec::reserve(values, vec::len(parsers));
	
	let mut i = 0u;
	let mut out = input;
	while i < vec::len(parsers)		// TODO: use vec::iter
	{
		alt parsers[i](out)
		{
			result::ok(answer)
			{
				out = answer;
				vec::push(values, answer.value);
			}
			result::err(error)
			{
				ret plog("sequence", input, result::err({output: input with error}));
			}
		}
		i += 1u;
	}
	
	alt eval(values)
	{
		result::ok(value)
		{
			ret plog("sequence", input, result::ok({value: value with out}));
		}
		result::err(mesg)
		{
			ret plog("sequence", input, result::err({output: input, maxIndex: out.index, mesg: mesg}));
		}
	}
}

#[doc = "Parses with the aid of a pointer to a parser (useful for things like parenthesized expressions)."]
fn forward_ref<T: copy>(input: state<T>, parser: @mut parser<T>) -> status<T>
{
	ret (*parser)(input);
}

#[doc = "Parses the text and does not fail if all the text was not consumed.."]
fn just<T: copy>(file: str, parser: parser<T>, seed: T, text: str) -> status<T>
{
	#info["------------------------------------------"];
	#info["parsing '%s'", text];
	ret parser({file: file, text: chars_with_eot(text), index: 0u, line: 1, value: seed});
}

#[doc = "Parses the text and fails if all the text was not consumed. Leading space is allowed."]
fn everything<T: copy>(file: str, parser: parser<T>, space: parser<T>, seed: T, text: str) -> status<T>
{
	#info["------------------------------------------"];
	#info["parsing '%s'", text];
	let input = {file: file, text: chars_with_eot(text), index: 0u, line: 1, value: seed};
	ret sequence(input, [space, parser, eot(_)]) {|results| result::ok(results[2])};
}
