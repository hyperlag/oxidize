public class VarKeyword {
    public static void main(String[] args) {
        // var in simple assignment
        var x = 42;
        System.out.println(x);

        // var with string
        var greeting = "hello";
        System.out.println(greeting);

        // var with boolean
        var flag = true;
        System.out.println(flag);

        // var with arithmetic
        var n = 10;
        var doubled = n * 2;
        System.out.println(doubled);

        // var in for loop
        var sum = 0;
        for (var i = 1; i <= 5; i++) {
            sum += i;
        }
        System.out.println(sum);
    }
}
