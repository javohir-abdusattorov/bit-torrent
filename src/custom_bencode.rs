
fn decode( iterator: &mut Peekable<Chars<'_>> ) -> serde_json::Value {
    let indicator = iterator.next().unwrap();

    match indicator {
        // string
        length if length.is_digit(10) => {
            let rest = iterator
                .take_while(|c| *c != ':')
                .collect::<String>();

            let length = format!("{length}{rest}").parse::<usize>().unwrap();
            let data = iterator
                .take(length)
                .collect::<String>();

            serde_json::Value::String(data)
        }

        // integer
        'i' => {
            let number = iterator
                .take_while(|c| *c != 'e')
                .collect::<String>();

            serde_json::Value::Number(Number::from_str(&number).unwrap())
        }

        // list
        'l' => {
            let mut list = Vec::new();

            while *iterator.peek().unwrap() != 'e' {
                let element = decode(iterator);
                list.push(element);
            }

            serde_json::Value::Array(list)
        }

        // dictionary
        'd' => {
            let mut map = Map::new();

            while *iterator.peek().unwrap() != 'e' {
                let key = decode(iterator);
                let value = decode(iterator);

                // println!("{key} = {value}");
                map.insert(key.as_str().unwrap().to_string(), value);
            }

            serde_json::Value::Object(map)
        }

        // unknown
        indicator => panic!("Unhandled encoded value: {indicator:?}")
    }
}

fn encode( value: serde_json::Value ) -> Vec<u8> {
    let mut result = vec![];

    match value {
        serde_json::Value::String(str) => {
            result.append(&mut format!("{}:", str.len()).as_bytes().to_vec());
            result.append(&mut str.as_bytes().to_vec());
            result
        }

        serde_json::Value::Number(number) => {
            result.push(b'i');
            result.append(&mut format!("{number}").as_bytes().to_vec());
            result.push(b'e');
            result
        }

        serde_json::Value::Array(array) => {
            result.push(b'l');

            array
                .into_iter()
                .map(encode)
                .for_each(|mut buf| result.append(&mut buf));

            result.push(b'e');
            result
        }

        serde_json::Value::Object(object) => {
            result.push(b'l');

            let mut list = object
                .into_iter()
                .collect::<Vec<(String, serde_json::Value)>>();

            list.sort_by_key(|(key, _)| key.clone());

            list
                .into_iter()
                .map(|(key, value)| {
                    let key = encode(serde_json::Value::String(key));
                    let value = encode(value);
                    vec![&key[..], &value[..]].concat()
                })
                .for_each(|mut buf| result.append(&mut buf));

            result.push(b'e');
            result
        },

        value => panic!("Unhandled decoded value: {value:?}")
    }
}

fn to_sha1( buf: &[u8] ) -> String {
    println!("input: {buf:?}");
    let mut hasher = Sha1::new();
    hasher.update(buf);
    let result = hasher.finalize();

    println!("hex: {:x?}", result);
    println!("result: {:?}", String::from_utf8_lossy(&result.to_vec()));
    String::new()
}