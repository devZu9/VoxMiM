use std::collections::HashMap;

fn numerals() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("ноль", "0");
    m.insert("один", "1");
    m.insert("два", "2");
    m.insert("три", "3");
    m.insert("четыре", "4");
    m.insert("пять", "5");
    m.insert("шесть", "6");
    m.insert("семь", "7");
    m.insert("восемь", "8");
    m.insert("девять", "9");
    m.insert("десять", "10");
    m
}

fn operators() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("плюс", "+");
    m.insert("минус", "-");
    m.insert("умножить", "*");
    m.insert("разделить", "/");
    m.insert("равно", "=");
    m
}

pub fn convert_math(text: &str) -> String {
    let nums = numerals();
    let ops = operators();
    let mut result = text.to_string();

    for (rus, num) in &nums {
        result = result.replace(rus, num);
    }

    for (rus, op) in &ops {
        result = result.replace(rus, op);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_expression() {
        let result = convert_math("два плюс три");
        assert_eq!(result, "2 + 3");
    }

    #[test]
    fn test_complex() {
        let result = convert_math("два плюс три умножить на четыре");
        assert_eq!(result, "2 + 3 * на 4");
    }
}
