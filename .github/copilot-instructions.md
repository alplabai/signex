Bütün commit mesajları İngilizce olmalıdır. Mesajlar, yapılan değişikliğin ne olduğunu ve neden yapıldığını açıkça belirtmelidir. Mesajlar kısa ve öz olmalıdır, ancak yeterli bilgi içermelidir. Örneğin

```feat: Add new feature X
fix: Fix bug Y
docs: Update documentation for Z
refactor: Refactor code for better readability
test: Add tests for feature A
```

Bu format, yapılan değişikliklerin türünü ve amacını hızlıca anlamamıza yardımcı olur. Ayrıca, commit mesajlarında kullanılan fiillerin zamanına dikkat edin; genellikle geniş zaman kullanılır (örneğin, "Add" yerine "Added" değil).

Rust idiomlarına uygun olarak, fonksiyon ve değişken adları snake_case formatında olmalıdır. Örneğin:

````rustfn calculate_total_price() -> f64 {
    // function body
}
```Bu, Rust topluluğunda yaygın olarak kabul edilen bir konvansiyondur ve kodun okunabilirliğini artırır. Ayrıca, modüller ve paketler için de snake_case kullanılması önerilir. Örneğin:
```rustmod user_management {
    // module body
}```Bu, kodun tutarlı ve anlaşılır olmasını sağlar. Ancak, struct ve enum adları için CamelCase formatı kullanılması önerilir. Örneğin:
```ruststruct UserProfile {
    // struct body
}
````

Fonksiyon ve değişken adlarında anlamlı ve açıklayıcı isimler kullanmaya özen gösterin. Kısaltmalardan kaçının ve mümkün olduğunca açık olun. Örneğin, `calculate_total_price` fonksiyonu, `calc_price` gibi kısa bir isimden daha açıklayıcıdır. Bu, kodun okunabilirliğini artırır ve diğer geliştiricilerin kodu daha kolay anlamasına yardımcı olur.Ayrıca, fonksiyonların ve değişkenlerin ne işe yaradığını açıklayan yorumlar eklamaya özen gösterin. Yorumlar, kodun amacını ve işlevini açıklar ve diğer geliştiricilerin kodu daha kolay anlamasına yardımcı olur. Ancak, yorumların gereksiz veya aşırı detaylı olmamasına dikkat edin; kodun kendisi mümkün olduğunca açıklayıcı olmalıdır. Yorumlar, kodun karmaşıklığını azaltmak ve anlaşılmasını kolaylaştırmak için kullanılmalıdır. Örneğin:

````rust// This function calculates the total price of items in the cart
fn calculate_total_price(cart: &Cart) -> f64 {
    // function body
}```Bu, fonksiyonun amacını açıklar ve diğer geliştiricilerin kodu daha kolay anlamasına yardımcı olur. Ayrıca, kodun karmaşıklığını azaltmak için fonksiyonları küçük ve tek bir sorumluluğa sahip olacak şekilde tasarlamaya özen gösterin.


Tüm kod ingilizce yazılmalıdır. Değişken adları, fonksiyon adları, yorumlar ve diğer kod öğeleri İngilizce olmalıdır. Bu, kodun uluslararası bir geliştirici topluluğu tarafından daha kolay anlaşılmasını sağlar ve işbirliğini artırır. Ayrıca, İngilizce olmayan karakterler veya kelimeler kullanmaktan kaçının, çünkü bu kodun okunabilirliğini azaltabilir ve hata yapma olasılığını artırabilir. Örneğin:
```rustlet total_price = calculate_total_price(cart);
```Bu, kodun İngilizce olduğunu ve anlaşılır olduğunu gösterir. Ayrıca, kodun tutarlı bir şekilde İngilizce yazılması, diğer geliştiricilerin kodu daha kolay anlamasına ve katkıda bulunmasına yardımcı olur. Bu, özellikle uluslararası bir ekipte çalışırken önemlidir, çünkü farklı dillerde konuşan geliştiriciler arasında iletişimi kolaylaştırır.
````
Kendini co-author olarak ekleme sadece yazar görünsün.