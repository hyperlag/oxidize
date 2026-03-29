public class EnumCompare {
    enum Season { SPRING, SUMMER, FALL, WINTER }

    public static void main(String[] args) {
        Season a = Season.SUMMER;
        Season b = Season.SUMMER;
        Season c = Season.WINTER;
        System.out.println(a == b);
        System.out.println(a == c);
        System.out.println(a.equals(b));
        System.out.println(Season.valueOf("FALL").name());
    }
}
